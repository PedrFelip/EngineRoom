import type { ReviewResult } from '../../types'
import {
  ADAPTIVE_PROFILES,
  type AdaptiveProfileId,
  rankCriticalMoves,
  selectRefinementTargets,
} from '../adaptive-analysis'
import { lookupOpening } from '../eco'
import { computePhases } from '../phase'
import { sideToMoveAtPly, whiteWinPct } from '../scoring'
import { isReadyOk } from '../uci'
import {
  type AnalysisProgress,
  type AnalyzeControl,
  type BookInfo,
  defaultGoTimeout,
  type EnginePort,
  type PositionCache,
  type RawPosition,
} from './analysis-types'
import {
  addSanToLines,
  ask,
  configureEngine,
  evalPosition,
  extractGame,
  terminalCp,
  terminalCps,
  terminalPosition,
} from './engine-analysis'
import { buildReview } from './review-builder'

/**
 * Revisão adaptativa em duas passagens. A triagem cobre todas as posições com
 * MultiPV > 1; apenas pares antes/depois de lances críticos recebem uma busca
 * maior. O resultado final mistura posições de profundidades diferentes.
 */
export async function analyzeGameAdaptive(
  pgn: string,
  profileId: AdaptiveProfileId,
  port: EnginePort,
  opts: {
    threads?: number
    hashMb?: number
    cache?: PositionCache
    keepAlive?: boolean
    goTimeoutMs?: number
    onProgress?: (winPcts: number[]) => void
    onDetailedProgress?: (progress: AnalysisProgress) => void
  } = {},
): Promise<ReviewResult> {
  const profile = ADAPTIVE_PROFILES[profileId]
  const { positionFens, moves } = extractGame(pgn)
  const game = { startFen: positionFens[0], moves }
  const triageControl: AnalyzeControl = {
    mode: 'time',
    movetimeMs: profile.triageMs,
  }

  await configureEngine(port, {
    threads: opts.threads,
    hashMb: opts.hashMb,
    multipv: profile.triageMultipv,
  })

  const triageHits = opts.cache
    ? await opts.cache.getBulk(
        positionFens,
        'time',
        profile.triageMs,
        profile.triageMultipv,
      )
    : positionFens.map(() => null)
  const raw: RawPosition[] = []
  const triagePuts: RawPosition[] = []
  const winPcts: number[] = []
  const terminals = terminalCps(positionFens)
  const phases = computePhases(positionFens.map((fen) => ({ fen })))
  let cachedPositions = 0
  let enginePositions = 0
  let remainingTriagePositions = terminals.filter(
    (term, index) => term === null && !triageHits[index],
  ).length

  async function flushTriagePuts(): Promise<void> {
    if (!opts.cache || !triagePuts.length) return
    await opts.cache.putMany(
      triagePuts,
      'time',
      profile.triageMs,
      profile.triageMultipv,
    )
    triagePuts.length = 0
  }

  try {
    for (let index = 0; index < positionFens.length; index++) {
      const fen = positionFens[index]
      const term = terminals[index]
      const cached = triageHits[index]
      if (term === null && !cached) remainingTriagePositions--
      let pos: RawPosition
      if (term !== null) {
        pos = terminalPosition(fen, term)
      } else if (cached) {
        pos = cached
        cachedPositions++
      } else {
        pos = await evalPosition(
          port,
          fen,
          triageControl,
          opts.goTimeoutMs ?? defaultGoTimeout(triageControl),
        )
        addSanToLines(pos)
        enginePositions++
        triagePuts.push(pos)
        if (triagePuts.length >= 8) await flushTriagePuts()
      }
      raw.push(pos)
      winPcts.push(whiteWinPct(pos.cp, sideToMoveAtPly(moves, index)))
      opts.onProgress?.(winPcts.slice())
      opts.onDetailedProgress?.({
        stage: 'triage',
        completed: index + 1,
        total: positionFens.length,
        currentPly: index,
        phase: phases[index],
        winPcts: winPcts.slice(),
        cachedPositions,
        enginePositions,
        remainingBudgetMs: remainingTriagePositions * profile.triageMs,
      })
    }

    await flushTriagePuts()

    const opening = await lookupOpening(moves.map((move) => move.san))
    const book: BookInfo | undefined = opening
      ? { maxPly: opening.moves.length, eco: opening }
      : undefined
    const criticalMoves = rankCriticalMoves(moves, raw, book?.maxPly ?? 0)
    const targets = selectRefinementTargets(
      criticalMoves,
      positionFens.length,
      profile,
    )
    const refinementTargets = targets.filter(
      (target) => terminalCp(positionFens[target.positionIndex]) === null,
    )

    if (refinementTargets.length > 0) {
      if (profile.refinementMultipv !== profile.triageMultipv) {
        await port.send(
          `setoption name Multipv value ${profile.refinementMultipv}`,
        )
        await ask(port, 'isready', isReadyOk)
      }

      let refined = 0
      const refinementBudget = (target: (typeof refinementTargets)[number]) =>
        target.budget === 'high' ? profile.highMs : profile.mediumMs
      let remainingRefinementBudgetMs = refinementTargets.reduce(
        (sum, target) => sum + refinementBudget(target),
        0,
      )
      opts.onDetailedProgress?.({
        stage: 'refinement',
        completed: 0,
        total: refinementTargets.length,
        currentPly: refinementTargets[0].positionIndex,
        phase: phases[refinementTargets[0].positionIndex],
        winPcts: winPcts.slice(),
        cachedPositions,
        enginePositions,
        remainingBudgetMs: remainingRefinementBudgetMs,
      })

      for (const budget of ['high', 'medium'] as const) {
        const movetimeMs = budget === 'high' ? profile.highMs : profile.mediumMs
        const control: AnalyzeControl = { mode: 'time', movetimeMs }
        const group = refinementTargets.filter(
          (target) => target.budget === budget,
        )
        const refinementPuts: RawPosition[] = []
        const activeTargets = group.filter(
          (target) => terminals[target.positionIndex] === null,
        )
        const refinementHits =
          opts.cache && activeTargets.length > 0
            ? await opts.cache.getBulk(
                activeTargets.map(
                  (target) => positionFens[target.positionIndex],
                ),
                'time',
                movetimeMs,
                profile.refinementMultipv,
              )
            : activeTargets.map(() => null)
        const hitByPosition = new Map(
          activeTargets.map((target, index) => [
            target.positionIndex,
            refinementHits[index],
          ]),
        )

        for (const target of group) {
          const index = target.positionIndex
          const fen = positionFens[index]
          if (terminals[index] !== null) continue
          let pos = hitByPosition.get(index) ?? null
          if (pos) {
            cachedPositions++
          } else {
            pos = await evalPosition(
              port,
              fen,
              control,
              opts.goTimeoutMs ?? defaultGoTimeout(control),
            )
            addSanToLines(pos)
            enginePositions++
            refinementPuts.push(pos)
          }
          raw[index] = pos
          winPcts[index] = whiteWinPct(pos.cp, sideToMoveAtPly(moves, index))
          refined++
          remainingRefinementBudgetMs -= movetimeMs
          opts.onProgress?.(winPcts.slice())
          opts.onDetailedProgress?.({
            stage: 'refinement',
            completed: refined,
            total: refinementTargets.length,
            currentPly: index,
            phase: phases[index],
            winPcts: winPcts.slice(),
            cachedPositions,
            enginePositions,
            remainingBudgetMs: remainingRefinementBudgetMs,
          })
        }

        if (opts.cache && refinementPuts.length) {
          await opts.cache.putMany(
            refinementPuts,
            'time',
            movetimeMs,
            profile.refinementMultipv,
          )
        }
      }
    }

    opts.onDetailedProgress?.({
      stage: 'finalizing',
      completed: positionFens.length,
      total: positionFens.length,
      currentPly: positionFens.length - 1,
      phase: phases[phases.length - 1] ?? 'opening',
      winPcts: winPcts.slice(),
      cachedPositions,
      enginePositions,
    })
    if (!opts.keepAlive) await port.send('quit')
    return buildReview(game, raw, book)
  } catch (err) {
    if (triagePuts.length) {
      try {
        await flushTriagePuts()
      } catch (flushErr) {
        console.warn(
          'Falha ao descarregar a triagem adaptativa após aborto:',
          flushErr,
        )
      }
    }
    throw err
  }
}
