import { Effect, Layer } from 'effect'
import { afterEach } from 'vitest'
import type { ReviewConfig, ReviewResult } from '../../../types'
import {
  PositionCache as CacheService,
  Engine,
  GamesRepository,
  SystemResources,
} from '../../__tests__/reference/backend'
import {
  type MountedReviewSession,
  mountReviewSession,
} from '../../__tests__/reference/effect/ui-runtime'
import type { ReviewSessionOpts } from '../../__tests__/reference/review-session'
import {
  type EnginePort,
  effectCache,
  effectPort,
  type PositionCache,
} from '../../analysis/__tests__/analyze-test-runtime'
import { bestEffort } from '../../effect/diagnostics'
import {
  EngineSpawnError,
  errorMessage,
  PersistenceError,
  SystemResourcesError,
} from '../../effect/errors'
import type { SystemResources as Resources } from '../../system'

export type EnginePortHandle = EnginePort & { dispose(): Promise<void> }
export interface Backend {
  createEnginePort(isCancelled: () => boolean): Promise<EnginePortHandle | null>
  createPositionCache(): PositionCache
  getSystemResources(): Promise<Resources>
  saveReview(config: ReviewConfig, result: ReviewResult): Promise<number>
}
const sessions = new Set<MountedReviewSession>()
afterEach(async () => {
  await Promise.all([...sessions].map((session) => session.dispose()))
  sessions.clear()
})
export function fakeLayer(backend: Backend) {
  return Layer.mergeAll(
    Layer.succeed(Engine, {
      acquire: Effect.acquireRelease(
        Effect.tryPromise({
          try: () => backend.createEnginePort(() => false),
          catch: (cause) =>
            new EngineSpawnError({ cause, message: errorMessage(cause) }),
        }),
        (port) =>
          port
            ? bestEffort(
                Effect.promise(() => port.dispose()),
                'test.dispose',
              )
            : Effect.void,
      ).pipe(
        Effect.flatMap((port) =>
          port ? Effect.succeed(effectPort(port)) : Effect.interrupt,
        ),
      ),
    }),
    Layer.succeed(CacheService, effectCache(backend.createPositionCache())),
    Layer.succeed(SystemResources, {
      get: Effect.tryPromise({
        try: () => backend.getSystemResources(),
        catch: (cause) =>
          new SystemResourcesError({ cause, message: errorMessage(cause) }),
      }),
    }),
    Layer.succeed(GamesRepository, {
      saveReview: (config, result) =>
        Effect.tryPromise({
          try: () => backend.saveReview(config, result),
          catch: (cause) =>
            new PersistenceError({
              operation: 'save',
              cause,
              message: errorMessage(cause),
            }),
        }),
    }),
  )
}
export function createReviewSession(
  opts: ReviewSessionOpts & { backend: Backend },
) {
  const session = mountReviewSession(opts, fakeLayer(opts.backend))
  sessions.add(session)
  return session
}
