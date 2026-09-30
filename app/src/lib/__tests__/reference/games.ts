import { Effect, Schema } from 'effect'
import type {
  GameCursor,
  PgnMeta,
  ReviewConfig,
  ReviewResult,
  StoredGame,
} from '../../../types'
import { errorMessage, PersistenceError } from '../../effect/errors'
import { decode, ipc } from '../../effect/ipc'
import {
  GamePageSchema,
  type LegacyReview,
  LegacyReviewSchema,
  SavedIdSchema,
  StoredGameSchema,
} from '../../effect/schemas'
import { resolveEngineTier } from '../../engine-tier'
import { parsePgn } from '../../pgn'
import { adaptiveProfileForKind } from './adaptive-analysis'
import { accuracyByPhaseOf } from './analyze'
import { computePhases } from './phase'
import {
  ACCURACY_MODEL_VERSION,
  centipawnLoss,
  classifyMove,
  gameAccuracy,
} from './scoring'

/** Página do histórico, da partida mais recente para a mais antiga. */
export function listGames(limit: number, cursor: GameCursor | null = null) {
  return gamesIpc('games_list', { limit, cursor }).pipe(
    Effect.flatMap((data) => decode(GamePageSchema, data, 'games_list')),
  )
}

/** Busca a partida completa (pgn + revisão) para reabertura instantânea. */
export function getGame(id: number) {
  return gamesIpc('games_get', { id }).pipe(
    Effect.flatMap((data) =>
      decode(Schema.NullOr(StoredGameSchema), data, 'games_get'),
    ),
  )
}

export function getReviewConfig(id: number) {
  return getGame(id).pipe(
    Effect.flatMap((game) =>
      game ? storedToConfig(game) : Effect.succeed(null),
    ),
  )
}

export function deleteGame(id: number) {
  return gamesIpc('games_delete', { id }).pipe(Effect.asVoid)
}

/** Esvazia todo o histórico de partidas revisadas (não toca no cache). */
export function clearGames() {
  return gamesIpc('games_clear').pipe(Effect.asVoid)
}

/**
 * Grava a revisão concluída no store. Reanálise da mesma partida com os
 * mesmos parâmetros (pgn, mode, depth/movetimeMs, multipv) substitui a
 * entrada anterior.
 */
export function saveReview(config: ReviewConfig, result: ReviewResult) {
  const analysisKind = config.analysisKind ?? 'manual'
  const adaptiveProfile = adaptiveProfileForKind(analysisKind)
  let controlValue = config.engine.depth
  if (adaptiveProfile) controlValue = adaptiveProfile.highMs
  else if (config.mode === 'time') controlValue = config.movetimeMs ?? 0
  return gamesIpc('games_save', {
    game: {
      pgn: config.pgn,
      white: config.meta.white,
      black: config.meta.black,
      result: config.meta.result,
      plies: config.meta.plies,
      engineTier: analysisKind === 'manual' ? config.engine.id : analysisKind,
      mode: config.mode,
      analysisKind,
      depth: controlValue,
      multipv: config.lines,
      accuracyWhite: result.accuracy.white,
      accuracyBlack: result.accuracy.black,
      reviewJson: JSON.stringify(result),
    },
  }).pipe(Effect.flatMap((data) => decode(SavedIdSchema, data, 'games_save')))
}

/**
 * Garante que uma revisão (possivelmente antiga, do store) tenha apenas
 * classificações atuais, `phase`, `cpLoss` e accuracy no modelo atual.
 * Recomputa a partir das avaliações já persistidas — puro e barato.
 */
function normalizeReview(result: LegacyReview): ReviewResult {
  const phases = computePhases(result.positions)
  const positions = result.positions.map((p, i) => ({
    ...p,
    phase: p.phase ?? phases[i],
  }))
  const moves = result.moves.map((move) => {
    const classification =
      move.classification === 'brilhante' || move.classification === 'otimo'
        ? classifyMove(move.winPctLoss, move.isBook)
        : move.classification
    const before = result.positions[move.ply - 1]
    const after = result.positions[move.ply]
    return {
      ...move,
      classification,
      cpLoss:
        move.cpLoss ??
        (before && after ? centipawnLoss(before.cp, after.cp) : 0),
    }
  })
  const current =
    result.accuracyModel === ACCURACY_MODEL_VERSION &&
    result.positions.every((p) => p.phase) &&
    result.moves.every(
      (m) =>
        m.cpLoss !== undefined &&
        m.classification !== 'brilhante' &&
        m.classification !== 'otimo',
    ) &&
    result.accuracyByPhase !== undefined
  const winPcts = positions.map((p) => p.winPct)
  return {
    positions,
    moves,
    accuracyModel: ACCURACY_MODEL_VERSION,
    accuracy: current ? result.accuracy : gameAccuracy(moves, winPcts),
    accuracyByPhase:
      current && result.accuracyByPhase
        ? result.accuracyByPhase
        : accuracyByPhaseOf(moves, phases, winPcts),
  }
}

/**
 * Converte uma partida do store em ReviewConfig com o resultado pré-carregado
 * (useReview pula a análise quando initialResult está presente).
 * Os metadados são reparseados do PGN — fonte única de verdade para
 * elo/evento, que o store não duplica.
 */
export function storedToConfig(game: StoredGame) {
  return decode(LegacyReviewSchema, game.reviewJson, 'revisão salva').pipe(
    Effect.map((review): ReviewConfig => {
      const mode = game.mode ?? 'depth'
      const analysisKind = game.analysisKind ?? 'manual'
      const movetimeMs =
        analysisKind === 'manual' && mode === 'time' ? game.depth : undefined
      const engine =
        mode === 'depth' ? resolveEngineTier(game.depth) : resolveEngineTier(20)

      const parsed = parsePgn(game.pgn)
      const meta: PgnMeta = parsed.ok
        ? parsed.meta
        : {
            white: game.white,
            black: game.black,
            whiteElo: null,
            blackElo: null,
            result: game.result,
            event: null,
            plies: game.plies,
          }

      return {
        pgn: game.pgn,
        meta,
        engine,
        mode,
        analysisKind,
        ...(movetimeMs !== undefined ? { movetimeMs } : {}),
        lines: game.multipv,
        initialResult: normalizeReview(review),
      }
    }),
  )
}

function gamesIpc(operation: string, args?: Record<string, unknown>) {
  return ipc(
    operation,
    args,
    (cause) =>
      new PersistenceError({
        operation,
        cause,
        message: errorMessage(cause),
      }),
  )
}
