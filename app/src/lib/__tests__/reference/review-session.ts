import {
  Cause,
  Chunk,
  Deferred,
  Effect,
  Exit,
  Fiber,
  Metric,
  Option,
  Queue,
  Scope,
} from 'effect'
/**
 * Sessão de revisão: orquestração de I/O de uma partida — boot da engine
 * (com sizing best-effort), análise nova ou reabertura do store e persistência.
 * Estado de navegação mora no `ReviewStore` injetado; aqui é só sequenciamento
 * sobre o seam `Backend`, testável com fakes (mesma disciplina do `EnginePort`).
 */

import type {
  Classification,
  Phase,
  PositionAnalysis,
  ReviewConfig,
} from '../../../types'
import { metrics, warn } from '../../effect/diagnostics'
import { type AnalysisError, errorMessage } from '../../effect/errors'
import type { ReviewStore } from '../../review-store'
import { recommendedReviewThreads } from '../../settings'
import { recommendedHashMb } from '../../system'
import { adaptiveProfileForKind } from './adaptive-analysis'
import type { RawPosition } from './analysis/analysis-types'
import {
  addSanToLines,
  ask,
  configureEngine,
  evalPosition,
  MissingEvaluationError,
  terminalCp,
} from './analysis/engine-analysis'
import type { EnginePort } from './analyze'
import {
  type AnalysisProgress,
  type AnalyzeControl,
  analyzeGame,
  analyzeGameAdaptive,
  type WinPctUpdate,
} from './analyze'
import {
  Engine,
  GamesRepository,
  PositionCache,
  SystemResources,
} from './backend'
import { phaseOfPosition } from './phase'
import { classifyMove, cpToWinPct, whiteCp, whiteWinPct } from './scoring'
import { isReadyOk } from './uci'

export interface ReviewSessionState {
  status: 'running' | 'done' | 'error'
  error: string | null
}

export interface ReviewProgress {
  stage: 'preparing' | AnalysisProgress['stage']
  completed: number
  total: number
  currentPly: number
  phase: Phase | null
  /** Buffer privado da sessão; a view deve criar seu snapshot antes de renderizar. */
  winPcts: readonly number[]
  cachedPositions: number
  enginePositions: number
  remainingBudgetMs?: number
}

export interface ReviewSessionOpts {
  config: ReviewConfig
  store: ReviewStore
  onStateChange(state: ReviewSessionState): void
  /** Progresso rico da engine — cru, antes do coalescing por rAF da view. */
  onProgress(progress: ReviewProgress): void
}

export interface ReviewSession {
  start(): Effect.Effect<void>
  analyzePosition(
    request: LiveAnalysisRequest,
    settings: LiveAnalysisSettings,
  ): Effect.Effect<void>
  cancelLiveAnalysis(): Effect.Effect<void>
}

export interface LiveAnalysisSettings {
  searchSeconds: number
  lines: number
  threadsAuto: boolean
  threads: number
  memoryMb: number
  moveFeedbackEnabled: boolean
  /** Durante a reprodução da PV, privilegia resposta imediata por lance. */
  fastPass?: boolean
}

interface ResolvedLiveAnalysisSettings
  extends Omit<LiveAnalysisSettings, 'threadsAuto'> {}

interface LiveSearchPlan {
  movetimeMs: number
  multipv: number
}

const PLAYBACK_SEARCH_MS = 500

export interface LiveAnalysisRequest {
  fen: string
  variationNodeId?: string
  sourceFen?: string
  sourceAnalysis?: PositionAnalysis
}

/** Synchronous restoration, also used before starting the UI runtime. */
export function restoreInitialReview(opts: ReviewSessionOpts): boolean {
  const result = opts.config.initialResult
  if (!result) return false
  if (opts.store.getState().result !== result) {
    opts.store.setResult(result)
    opts.onStateChange({ status: 'done', error: null })
  }
  return true
}

function manualAnalysisControl(config: ReviewConfig): AnalyzeControl {
  if (config.mode === 'time') {
    return { mode: 'time', movetimeMs: config.movetimeMs ?? 5000 }
  }
  return { mode: 'depth', depth: config.engine.depth }
}

/** The enclosing Scope owns dispatch, searches, persistence and engine leases. */
export function createReviewSession(opts: ReviewSessionOpts) {
  return Effect.gen(function* () {
    const { config, store } = opts
    const engineService = yield* Engine
    const cache = yield* PositionCache
    const games = yield* GamesRepository
    const resources = yield* SystemResources
    const scope = yield* Effect.scope
    const initialized = yield* Deferred.make<void>()
    const requests = yield* Queue.sliding<{
      request: LiveAnalysisRequest
      settings: LiveAnalysisSettings
    } | null>(1)
    yield* Effect.addFinalizer(() => Queue.shutdown(requests))
    const partialWinPcts: number[] = []
    let livePort: EnginePort | undefined
    let liveScope: Scope.CloseableScope | undefined
    let appliedLiveSettings: ResolvedLiveAnalysisSettings | undefined
    let detectedLiveResources: { threads: number; memoryMb: number } | undefined

    const discardLivePort = Effect.suspend(() => {
      const owned = liveScope
      liveScope = undefined
      livePort = undefined
      appliedLiveSettings = undefined
      return owned ? Scope.close(owned, Exit.void) : Effect.void
    })
    yield* Effect.addFinalizer(() => discardLivePort)

    const start = yield* Effect.once(
      Effect.gen(function* () {
        if (restoreInitialReview(opts)) return
        opts.onProgress({
          stage: 'preparing',
          completed: 0,
          total: config.meta.plies + 1,
          currentPly: 0,
          phase: null,
          winPcts: partialWinPcts,
          cachedPositions: 0,
          enginePositions: 0,
        })
        const review = yield* Effect.scoped(
          Effect.gen(function* () {
            const port = yield* engineService.acquire
            const sizing = yield* resources.get.pipe(
              Effect.map((r) => ({
                threads: r.threads,
                hashMb: recommendedHashMb(r.memory_mb),
              })),
              Effect.catchAll(() => Effect.succeed({})),
            )
            const analysisOpts = {
              ...sizing,
              cache,
              keepAlive: false,
              onDetailedProgress(
                progress: AnalysisProgress,
                update?: WinPctUpdate,
              ) {
                if (update) partialWinPcts[update.index] = update.winPct
                opts.onProgress({ ...progress, winPcts: partialWinPcts })
              },
            }
            const profile = adaptiveProfileForKind(config.analysisKind)
            const analysis = profile
              ? analyzeGameAdaptive(config.pgn, profile.id, port, analysisOpts)
              : analyzeGame(
                  config.pgn,
                  manualAnalysisControl(config),
                  port,
                  config.lines,
                  analysisOpts,
                )
            return yield* analysis
          }),
        )
        store.setResult(review)
        opts.onStateChange({ status: 'done', error: null })
        yield* games.saveReview(config, review).pipe(
          Effect.tapError(() => Metric.increment(metrics.persistenceFailures)),
          Effect.catchAll((error) => warn('games.save', error)),
          Effect.forkIn(scope),
        )
      }).pipe(
        Effect.onInterrupt(() => Metric.increment(metrics.interruptions)),
        Effect.catchAllCause((cause) =>
          Effect.sync(() => {
            if (!Cause.isInterrupted(cause)) {
              const failure = Cause.failureOption(cause)
              opts.onStateChange({
                status: 'error',
                error: Option.isSome(failure)
                  ? errorMessage(failure.value)
                  : Cause.pretty(cause),
              })
            }
          }),
        ),
        Effect.ensuring(Deferred.succeed(initialized, undefined)),
      ),
    )

    function ensureLivePort(
      settings: ResolvedLiveAnalysisSettings,
      plan: LiveSearchPlan,
    ) {
      return Effect.gen(function* () {
        if (!livePort) {
          liveScope = yield* Scope.make()
          livePort = yield* engineService.acquire.pipe(
            Effect.provideService(Scope.Scope, liveScope),
          )
          yield* configureEngine(livePort, {
            threads: settings.threads,
            hashMb: settings.memoryMb,
            multipv: plan.multipv,
          })
          appliedLiveSettings = settings
          return livePort
        }
        if (
          appliedLiveSettings?.threads !== settings.threads ||
          appliedLiveSettings.memoryMb !== settings.memoryMb ||
          appliedLiveSettings.lines !== settings.lines ||
          appliedLiveSettings.fastPass !== settings.fastPass
        ) {
          yield* livePort.send(
            `setoption name Threads value ${settings.threads}`,
          )
          yield* livePort.send(`setoption name Hash value ${settings.memoryMb}`)
          yield* livePort.send(`setoption name MultiPV value ${plan.multipv}`)
          yield* ask(livePort, 'isready', isReadyOk)
          appliedLiveSettings = settings
        }
        return livePort
      }).pipe(Effect.onError(() => discardLivePort))
    }

    function analyzeFen(
      fen: string,
      settings: ResolvedLiveAnalysisSettings,
    ): Effect.Effect<PositionAnalysis, AnalysisError> {
      return Effect.gen(function* () {
        const plan = liveSearchPlan(settings)
        let raw = yield* cache.get(fen, 'time', plan.movetimeMs, plan.multipv)
        if (!raw) {
          const terminal = terminalCp(fen)
          if (terminal !== null) {
            raw = {
              fen,
              cp: terminal,
              depth: 0,
              pv: [],
              lines: [{ multipv: 1, cp: terminal, pv: [] }],
            }
          } else {
            const port = yield* ensureLivePort(settings, plan)
            let attempt = 0
            raw = yield* Effect.gen(function* () {
              if (attempt++ > 0) {
                yield* Metric.increment(metrics.retries)
                yield* ask(port, 'isready', isReadyOk)
              }
              return yield* evalPosition(
                port,
                fen,
                { mode: 'time', movetimeMs: plan.movetimeMs },
                plan.movetimeMs + 10_000,
              )
            }).pipe(
              Effect.retry({
                times: 1,
                while: (error) => error instanceof MissingEvaluationError,
              }),
              // Never reuse a process with an unconsumed bestmove after interruption.
              Effect.onError(() => discardLivePort),
            )
            addSanToLines(raw)
            yield* cache.put(raw, 'time', plan.movetimeMs, plan.multipv)
          }
        }
        return {
          ...positionAnalysis(raw),
          search: {
            purpose: settings.fastPass ? 'playback' : 'refinement',
            movetimeMs: plan.movetimeMs,
            multipv: plan.multipv,
          },
        }
      })
    }

    function resolveLiveSettings(settings: LiveAnalysisSettings) {
      return Effect.gen(function* () {
        const manual = manualLiveSettings(settings)
        if (!settings.threadsAuto) return manual
        if (!detectedLiveResources) {
          const detected = yield* Effect.option(resources.get)
          if (Option.isNone(detected)) return manual
          detectedLiveResources = {
            threads: recommendedReviewThreads(detected.value.threads),
            memoryMb: recommendedHashMb(detected.value.memory_mb),
          }
        }
        return { ...manual, ...detectedLiveResources }
      })
    }

    function runLiveAnalysis(
      request: LiveAnalysisRequest,
      settings: LiveAnalysisSettings,
    ) {
      return Effect.gen(function* () {
        yield* Deferred.await(initialized)
        store.startLiveAnalysis(request.fen)
        const resolved = yield* resolveLiveSettings(settings)
        const analysis = yield* analyzeFen(request.fen, resolved)
        store.setLiveAnalysis(request.fen, analysis)
        if (
          !settings.moveFeedbackEnabled ||
          !request.variationNodeId ||
          !request.sourceFen
        )
          return
        let source =
          request.sourceAnalysis?.fen === request.sourceFen
            ? request.sourceAnalysis
            : undefined
        if (!source && request.sourceFen === request.fen) source = analysis
        if (!source && !settings.fastPass)
          source = yield* analyzeFen(request.sourceFen, resolved)
        if (source)
          store.setVariationClassification(
            request.variationNodeId,
            classifyLiveMove(source, analysis),
          )
      }).pipe(
        Effect.onInterrupt(() => Metric.increment(metrics.interruptions)),
        Effect.catchAllCause((cause) =>
          Effect.sync(() => {
            if (Cause.isInterrupted(cause)) return
            const failure = Cause.failureOption(cause)
            store.failLiveAnalysis(
              request.fen,
              Option.isSome(failure)
                ? errorMessage(failure.value)
                : Cause.pretty(cause),
            )
          }),
        ),
      )
    }

    yield* Effect.gen(function* () {
      let current: Fiber.RuntimeFiber<void, never> | undefined
      while (true) {
        let next = yield* Queue.take(requests)
        if (current) yield* Fiber.interrupt(current)
        // While finalizers were running, keep only the most recent intent.
        const queued = yield* Queue.takeAll(requests)
        const latest = Chunk.last(queued)
        if (Option.isSome(latest)) next = latest.value
        current = next
          ? yield* runLiveAnalysis(next.request, next.settings).pipe(
              Effect.forkScoped,
            )
          : undefined
      }
    }).pipe(Effect.forkScoped)

    return {
      start: () => start,
      analyzePosition: (request, settings) =>
        Queue.offer(requests, { request, settings }).pipe(Effect.asVoid),
      cancelLiveAnalysis: () =>
        Effect.gen(function* () {
          store.cancelLiveAnalysis()
          yield* Queue.offer(requests, null)
        }),
    } satisfies ReviewSession
  })
}

function manualLiveSettings(
  settings: LiveAnalysisSettings,
): ResolvedLiveAnalysisSettings {
  return {
    searchSeconds: settings.searchSeconds,
    lines: settings.lines,
    threads: settings.threads,
    memoryMb: settings.memoryMb,
    moveFeedbackEnabled: settings.moveFeedbackEnabled,
    fastPass: settings.fastPass,
  }
}

function liveSearchPlan(
  settings: ResolvedLiveAnalysisSettings,
): LiveSearchPlan {
  if (settings.fastPass) {
    return { movetimeMs: PLAYBACK_SEARCH_MS, multipv: 1 }
  }
  return {
    movetimeMs: settings.searchSeconds * 1000,
    multipv: settings.lines,
  }
}

function positionAnalysis(raw: RawPosition): PositionAnalysis {
  const stm = raw.fen.split(' ')[1] === 'b' ? 'b' : 'w'
  const lines = (raw.lines ?? []).map((line) => ({
    multipv: line.multipv,
    san: line.san ?? null,
    cp: whiteCp(line.cp, stm),
    winPct: whiteWinPct(line.cp, stm),
    pv: line.pv,
  }))
  return {
    ply: 0,
    fen: raw.fen,
    phase: phaseOfPosition(raw.fen),
    depth: raw.depth,
    cp: raw.cp,
    winPct: whiteWinPct(raw.cp, stm),
    pv: raw.pv,
    lines,
  }
}

function classifyLiveMove(
  before: PositionAnalysis,
  after: PositionAnalysis,
): Classification {
  const winPctBefore = cpToWinPct(before.cp)
  const winPctAfter = 100 - cpToWinPct(after.cp)
  const loss = Math.max(0, winPctBefore - winPctAfter)
  return classifyMove(loss)
}
