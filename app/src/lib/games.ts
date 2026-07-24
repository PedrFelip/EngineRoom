import { invoke } from '@tauri-apps/api/core'
import type {
  GameSummary,
  PgnMeta,
  ReviewConfig,
  ReviewResult,
  StoredGame,
} from '../types'
import { accuracyByPhaseOf } from './analyze'
import { resolveEngineTier } from './engine-tier'
import { computePhases } from './phase'
import { parsePgn } from './pgn'

/** Lista as partidas analisadas, da mais recente para a mais antiga. */
export function listGames(): Promise<GameSummary[]> {
  return invoke('games_list')
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
  const controlValue =
    config.mode === 'time' ? (config.movetimeMs ?? 0) : config.engine.depth
  return invoke('games_save', {
    game: {
      pgn: config.pgn,
      white: config.meta.white,
      black: config.meta.black,
      result: config.meta.result,
      plies: config.meta.plies,
      engineTier: config.engine.id,
      mode: config.mode,
      depth: controlValue,
      multipv: config.lines,
      accuracyWhite: result.accuracy.white,
      accuracyBlack: result.accuracy.black,
      reviewJson: JSON.stringify(result),
    },
  })
}

/**
 * Garante que uma revisão (possivelmente antiga, do store) tenha `phase` em
 * cada posição e `accuracyByPhase`. Recomputa a partir dos FENs/lances já
 * presentes — puro e barato. Partidas novas (já com fases) passam ilesas.
 */
function normalizeReview(result: ReviewResult): ReviewResult {
  const hasPhases = result.positions.every((p) => p.phase)
  if (hasPhases && result.accuracyByPhase) return result
  const phases = computePhases(result.positions)
  const positions = result.positions.map((p, i) => ({
    ...p,
    phase: p.phase ?? phases[i],
  }))
  return {
    ...result,
    positions,
    accuracyByPhase:
      result.accuracyByPhase ?? accuracyByPhaseOf(result.moves, phases),
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
  const movetimeMs = mode === 'time' ? game.depth : undefined
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
    ...(movetimeMs !== undefined ? { movetimeMs } : {}),
    lines: game.multipv,
    initialResult: normalizeReview(JSON.parse(game.reviewJson) as ReviewResult),
  }
}
