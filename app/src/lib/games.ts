import { invoke } from '@tauri-apps/api/core'
import type {
  GameCursor,
  GamePage,
  PgnMeta,
  ReviewConfig,
  ReviewResult,
  StoredGame,
} from '../types'
import { adaptiveProfileForKind } from './adaptive-analysis'
import { accuracyByPhaseOf } from './analyze'
import { resolveEngineTier } from './engine-tier'
import { parsePgn } from './pgn'
import { computePhases } from './phase'
import {
  ACCURACY_MODEL_VERSION,
  centipawnLoss,
  classifyMove,
  gameAccuracy,
} from './scoring'

/** Página do histórico, da partida mais recente para a mais antiga. */
export function listGames(
  limit: number,
  cursor: GameCursor | null = null,
): Promise<GamePage> {
  return invoke('games_list', { limit, cursor })
}

/** Busca a partida completa (pgn + revisão) para reabertura instantânea. */
export function getGame(id: number): Promise<StoredGame | null> {
  return invoke('games_get', { id })
}

export function deleteGame(id: number): Promise<void> {
  return invoke('games_delete', { id })
}

/** Esvazia todo o histórico de partidas revisadas (não toca no cache). */
export function clearGames(): Promise<void> {
  return invoke('games_clear')
}

/**
 * Grava a revisão concluída no store. Reanálise da mesma partida com os
 * mesmos parâmetros (pgn, mode, depth/movetimeMs, multipv) substitui a
 * entrada anterior.
 */
export function saveReview(
  config: ReviewConfig,
  result: ReviewResult,
): Promise<number> {
  const analysisKind = config.analysisKind ?? 'manual'
  const adaptiveProfile = adaptiveProfileForKind(analysisKind)
  let controlValue = config.engine.depth
  if (adaptiveProfile) controlValue = adaptiveProfile.highMs
  else if (config.mode === 'time') controlValue = config.movetimeMs ?? 0
  return invoke('games_save', {
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
  })
}

/**
 * Garante que uma revisão (possivelmente antiga, do store) tenha apenas
 * classificações atuais, `phase`, `cpLoss` e accuracy no modelo atual.
 * Recomputa a partir das avaliações já persistidas — puro e barato.
 */
function normalizeReview(result: ReviewResult): ReviewResult {
  const hasPhases = result.positions.every((p) => p.phase)
  const hasCpLoss = result.moves.every((m) => Number.isFinite(m.cpLoss))
  const hasCurrentClassifications = result.moves.every((move) => {
    const classification = move.classification as string
    return classification !== 'brilhante' && classification !== 'otimo'
  })
  const hasCurrentAccuracy = result.accuracyModel === ACCURACY_MODEL_VERSION
  if (
    hasPhases &&
    result.accuracyByPhase &&
    hasCpLoss &&
    hasCurrentClassifications &&
    hasCurrentAccuracy
  ) {
    return result
  }
  const phases = computePhases(result.positions)
  const positions = result.positions.map((p, i) => ({
    ...p,
    phase: p.phase ?? phases[i],
  }))
  const moves = result.moves.map((move) => {
    const legacyClassification = move.classification as string
    const classification =
      legacyClassification === 'brilhante' || legacyClassification === 'otimo'
        ? classifyMove(move.winPctLoss, move.isBook)
        : move.classification
    if (Number.isFinite(move.cpLoss)) return { ...move, classification }
    const before = result.positions[move.ply - 1]
    const after = result.positions[move.ply]
    return {
      ...move,
      classification,
      cpLoss: before && after ? centipawnLoss(before.cp, after.cp) : 0,
    }
  })
  const positionWinPcts = positions.map((position) => position.winPct)
  const accuracy = gameAccuracy(moves, positionWinPcts)
  return {
    ...result,
    positions,
    moves,
    accuracyModel: ACCURACY_MODEL_VERSION,
    accuracy,
    accuracyByPhase: accuracyByPhaseOf(moves, phases, positionWinPcts),
  }
}

/**
 * Converte uma partida do store em ReviewConfig com o resultado pré-carregado
 * (useReview pula a análise quando initialResult está presente).
 * Os metadados são reparseados do PGN — fonte única de verdade para
 * elo/evento, que o store não duplica.
 */
export function storedToConfig(game: StoredGame): ReviewConfig {
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
    initialResult: normalizeReview(JSON.parse(game.reviewJson) as ReviewResult),
  }
}
