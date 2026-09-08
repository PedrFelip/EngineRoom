import {
  Cause,
  Deferred,
  Effect,
  Exit,
  Fiber,
  type Layer,
  Option,
} from 'effect'
import type { Backend } from '../backend'
import {
  createReviewSession,
  type ReviewSession,
  type ReviewSessionOpts,
  restoreInitialReview,
} from '../review-session'
import { warn } from './diagnostics'

/** The UI/test boundary is the only place converting Effects into Promises.
 * Preserve domain errors, rather than leaking FiberFailure into UI messages. */
export async function runUi<A, E>(
  effect: Effect.Effect<A, E>,
  signal?: AbortSignal,
): Promise<A> {
  const exit = await Effect.runPromiseExit(effect, { signal })
  if (Exit.isSuccess(exit)) return exit.value
  const failure = Cause.failureOption(exit.cause)
  if (Option.isSome(failure)) throw failure.value
  throw new Error(Cause.pretty(exit.cause))
}

export interface MountedReviewSession {
  start(): Promise<void>
  analyzePosition: (
    ...args: Parameters<ReviewSession['analyzePosition']>
  ) => void
  cancelLiveAnalysis(): void
  dispose(): Promise<void>
}

/** One root fiber owns the complete mounted session, including background saves.
 * Commands only enqueue synchronous intent; asynchronous work stays in the scope. */
export function mountReviewSession(
  opts: ReviewSessionOpts,
  backend: Layer.Layer<Backend>,
): MountedReviewSession {
  const started = Effect.runSync(Deferred.make<void>())
  let root: Fiber.RuntimeFiber<void, never> | undefined
  let session: ReviewSession | undefined
  let pending: Parameters<ReviewSession['analyzePosition']> | null | undefined
  let disposed = false
  let closing: Promise<void> | undefined
  return {
    start() {
      if (disposed) return Promise.resolve()
      if (!root) {
        restoreInitialReview(opts)
        root = Effect.runFork(
          Effect.scoped(
            Effect.gen(function* () {
              session = yield* createReviewSession(opts)
              if (pending !== undefined) {
                yield* pending === null
                  ? session.cancelLiveAnalysis()
                  : session.analyzePosition(...pending)
                pending = undefined
              }
              yield* session.start()
              yield* Deferred.succeed(started, undefined)
              yield* Effect.never
            }),
          ).pipe(
            Effect.provide(backend),
            Effect.catchAllCause((cause) =>
              Cause.isInterruptedOnly(cause)
                ? Effect.void
                : warn('review.runtime', cause),
            ),
            Effect.ensuring(Deferred.succeed(started, undefined)),
          ),
        )
      }
      return runUi(Deferred.await(started))
    },
    analyzePosition(...args) {
      if (disposed) return
      if (session) Effect.runSync(session.analyzePosition(...args))
      else pending = args
    },
    cancelLiveAnalysis() {
      if (disposed) return
      if (session) Effect.runSync(session.cancelLiveAnalysis())
      else {
        opts.store.cancelLiveAnalysis()
        pending = null
      }
    },
    dispose() {
      disposed = true
      closing ??= root
        ? runUi(Fiber.interrupt(root).pipe(Effect.asVoid))
        : Promise.resolve()
      return closing
    },
  }
}
