import type { ReviewResult } from '../../types'
import { lookupOpening } from '../eco'
import { computePhases } from '../phase'
import { sideToMoveAtPly, whiteWinPct } from '../scoring'
import {
  type AnalysisProgress,
  type AnalyzeControl,
  type BookInfo,
  controlKeyValue,
  defaultGoTimeout,
  type EnginePort,
  type PositionCache,
  type RawPosition,
  type WinPctUpdate,
} from './analysis-types'
import {
  configureEngine,
  evalPosition,
  extractGame,
  terminalCps,
  uciToSan,
} from './engine-analysis'
import { buildReview } from './review-builder'

/** Analisa todas as posições com controle fixo e devolve a revisão completa. */
export async function analyzeGame(
  pgn: string,
  control: AnalyzeControl,
  port: EnginePort,
  multipv = 1,
  opts: {
    threads?: number
    hashMb?: number
    cache?: PositionCache
    /** Quando true, não envia `quit` ao final — a engine fica viva para refino ao vivo. */
    keepAlive?: boolean
    /** Override do timeout do `go` por posição (default via `defaultGoTimeout`). */
    goTimeoutMs?: number
    /** Atualização de progresso e, ao avaliar uma posição, seu win% pontual. */
    onDetailedProgress?: (
      progress: AnalysisProgress,
      update?: WinPctUpdate,
    ) => void
  } = {},
): Promise<ReviewResult> {
  const { positionFens, moves } = extractGame(pgn)
  const game = { startFen: positionFens[0], moves }
  const keyValue = controlKeyValue(control)
  const goTimeoutMs = opts.goTimeoutMs ?? defaultGoTimeout(control)

  await configureEngine(port, {
    threads: opts.threads,
    hashMb: opts.hashMb,
    multipv,
  })

  const hits = opts.cache
    ? await opts.cache.getBulk(positionFens, control.mode, keyValue, multipv)
    : positionFens.map(() => null)
  const pendingPuts: RawPosition[] = []
  const raw: RawPosition[] = []
  const terminals = terminalCps(positionFens)
  const phases = computePhases(positionFens.map((fen) => ({ fen })))
  let cachedPositions = 0
  let enginePositions = 0
  let remainingTimedPositions =
    control.mode === 'time'
      ? terminals.filter((term, index) => term === null && !hits[index]).length
      : 0
  try {
    for (let i = 0; i < positionFens.length; i++) {
      const fen = positionFens[i]
      const term = terminals[i]
      if (term === null && !hits[i]) remainingTimedPositions--
      let pos: RawPosition
      if (term !== null) {
        pos = {
          fen,
          cp: term,
          depth: 0,
          pv: [],
          lines: [{ multipv: 1, cp: term, pv: [] }],
        }
      } else {
        const cached = hits[i]
        if (cached) {
          pos = cached
          cachedPositions++
        } else {
          pos = await evalPosition(port, fen, control, goTimeoutMs)
          enginePositions++
          for (const l of pos.lines ?? []) {
            l.san = l.pv[0] ? uciToSan(pos.fen, l.pv[0]) : null
          }
          pendingPuts.push(pos)
          // Flush incremental: limita a perda num crash do processo a ~8
          // posições. Falha propaga (caminho crítico) e cai no catch abaixo,
          // cujo retry best-effort do buffer restante preserva a causa raiz.
          if (opts.cache && pendingPuts.length >= 8) {
            await opts.cache.putMany(
              pendingPuts,
              control.mode,
              keyValue,
              multipv,
            )
            pendingPuts.length = 0
          }
        }
      }
      raw.push(pos)
      const winPct = whiteWinPct(pos.cp, sideToMoveAtPly(game.moves, i))
      opts.onDetailedProgress?.(
        {
          stage: 'analyzing',
          completed: i + 1,
          total: positionFens.length,
          currentPly: i,
          phase: phases[i],
          cachedPositions,
          enginePositions,
          ...(control.mode === 'time'
            ? {
                remainingBudgetMs: remainingTimedPositions * control.movetimeMs,
              }
            : {}),
        },
        { index: i, winPct },
      )
    }
  } catch (err) {
    // Descarrega o buffer mesmo se a análise abortar no meio: posições já
    // avaliadas não se perdem — mas em caráter best-effort, para a causa
    // raiz do aborto vencer (a falha do flush vira warning).
    if (opts.cache && pendingPuts.length) {
      try {
        await opts.cache.putMany(pendingPuts, control.mode, keyValue, multipv)
      } catch (flushErr) {
        console.warn(
          'Falha ao descarregar o cache após aborto da análise:',
          flushErr,
        )
      }
    }
    throw err
  }
  if (opts.cache && pendingPuts.length) {
    // Caminho de sucesso: o cache é caminho crítico, não best-effort.
    await opts.cache.putMany(pendingPuts, control.mode, keyValue, multipv)
  }
  opts.onDetailedProgress?.({
    stage: 'finalizing',
    completed: positionFens.length,
    total: positionFens.length,
    currentPly: positionFens.length - 1,
    phase: phases[phases.length - 1] ?? 'opening',
    cachedPositions,
    enginePositions,
  })
  if (!opts.keepAlive) await port.send('quit')

  const opening = await lookupOpening(moves.map((m) => m.san))
  const book: BookInfo | undefined = opening
    ? { maxPly: opening.moves.length, eco: opening }
    : undefined

  return buildReview(game, raw, book)
}
