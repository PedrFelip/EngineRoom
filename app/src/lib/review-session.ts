import { Deferred, Effect, Queue } from 'effect'
import type { Phase, PositionAnalysis, ReviewConfig } from '../types'
import { AnalysisSessions } from './backend'
import { warn } from './effect/diagnostics'

export interface ReviewSessionState {
  status: 'running' | 'done' | 'error'
  error: string | null
}
export interface ReviewProgress {
  stage: 'preparing' | 'analyzing' | 'triage' | 'refinement' | 'finalizing'
  completed: number
  total: number
  currentPly: number
  phase: Phase | null
  winPcts: readonly number[]
  cachedPositions: number
  enginePositions: number
  remainingBudgetMs?: number
}
export interface ReviewSessionOpts {
  config: ReviewConfig
  store: import('./review-store').ReviewStore
  onStateChange(state: ReviewSessionState): void
  onProgress(progress: ReviewProgress): void
}
export interface LiveAnalysisSettings {
  searchSeconds: number
  lines: number
  threadsAuto: boolean
  threads: number
  memoryMb: number
  moveFeedbackEnabled: boolean
  fastPass?: boolean
}
export interface LiveAnalysisRequest {
  fen: string
  variationNodeId?: string
  sourceFen?: string
  sourceAnalysis?: PositionAnalysis
}
export interface ReviewSession {
  start(): Effect.Effect<void>
  analyzePosition(
    request: LiveAnalysisRequest,
    settings: LiveAnalysisSettings,
  ): Effect.Effect<void>
  cancelLiveAnalysis(): Effect.Effect<void>
}
export function restoreInitialReview(opts: ReviewSessionOpts): boolean {
  const result = opts.config.initialResult
  if (!result) return false
  if (opts.store.getState().result !== result) {
    opts.store.setResult(result)
    opts.onStateChange({ status: 'done', error: null })
  }
  return true
}

/** Scoped IPC and store glue. All search and persistence decisions are in Rust. */
export function createReviewSession(opts: ReviewSessionOpts) {
  return Effect.gen(function* () {
    const service = yield* AnalysisSessions
    const scope = yield* Effect.scope
    const ready = yield* Deferred.make<void>()
    let requestId = 0
    let disposed = false
    const winPcts: number[] = []
    const fail = (message: string) => {
      if (disposed) return
      opts.onStateChange({ status: 'error', error: message })
      Deferred.unsafeDone(ready, Effect.void)
    }
    const port = yield* service
      .open(
        opts.config,
        (event) => {
          if (disposed) return
          if (event.requestId !== undefined && event.requestId !== requestId)
            return
          switch (event.type) {
            case 'progress': {
              const { update, ...progress } = event.progress
              if (update) winPcts[update.index] = update.winPct
              opts.onProgress({ ...progress, winPcts })
              break
            }
            case 'completed':
              if (!opts.config.initialResult) opts.store.setResult(event.result)
              opts.onStateChange({ status: 'done', error: null })
              Deferred.unsafeDone(ready, Effect.void)
              break
            case 'liveStarted':
              opts.store.startLiveAnalysis(event.fen)
              break
            case 'liveCompleted':
              opts.store.setLiveAnalysis(event.fen, event.analysis)
              break
            case 'classification':
              opts.store.setVariationClassification(
                event.nodeId,
                event.classification,
              )
              break
            case 'error':
              if (event.fen)
                opts.store.failLiveAnalysis(event.fen, event.error.message)
              else fail(event.error.message)
              break
            case 'warning':
              Effect.runFork(warn(event.error.operation, event.error.message), {
                scope,
              })
              break
          }
        },
        (error) => fail(error.message),
      )
      .pipe(
        Effect.catchAll((error) =>
          Effect.sync(() => {
            fail(error.message)
            return null
          }),
        ),
      )
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        disposed = true
      }),
    )
    const intents = yield* Queue.sliding<{
      id: number
      request?: LiveAnalysisRequest
      settings?: LiveAnalysisSettings
    }>(1)
    yield* Effect.addFinalizer(() => Queue.shutdown(intents))
    yield* Effect.gen(function* () {
      while (true) {
        const intent = yield* Queue.take(intents)
        if (!port) continue
        const command =
          intent.request && intent.settings
            ? port.analyzePosition(intent.id, intent.request, intent.settings)
            : port.cancelLive(intent.id)
        yield* command.pipe(
          Effect.catchAll((error) =>
            Effect.sync(() => {
              if (disposed || intent.id !== requestId) return
              if (intent.request)
                opts.store.failLiveAnalysis(intent.request.fen, error.message)
            }),
          ),
          Effect.forkScoped,
        )
      }
    }).pipe(Effect.forkScoped)
    return {
      start: () => Deferred.await(ready),
      analyzePosition: (request, settings) =>
        Effect.gen(function* () {
          const id = ++requestId
          opts.store.startLiveAnalysis(request.fen)
          yield* Queue.offer(intents, { id, request, settings })
        }),
      cancelLiveAnalysis: () =>
        Effect.gen(function* () {
          const id = ++requestId
          opts.store.cancelLiveAnalysis()
          yield* Queue.offer(intents, { id })
        }),
    } satisfies ReviewSession
  })
}
