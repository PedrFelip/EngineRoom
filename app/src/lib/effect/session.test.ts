import { Deferred, Effect, Layer, Logger, TestClock, TestContext } from 'effect'
import { describe, expect, it, vi } from 'vitest'
import type { PositionCache as CachePort, EnginePort } from '../analyze'
import {
  Engine,
  GamesRepository,
  PositionCache,
  SystemResources,
} from '../backend'
import {
  depthConfig,
  existingResult,
} from '../review/__tests__/review-session-test-helpers'
import {
  createReviewSession,
  type LiveAnalysisSettings,
  type ReviewSessionState,
} from '../review-session'
import { createReviewStore } from '../review-store'
import { diagnosticsSnapshot } from './diagnostics'
import { PersistenceError } from './errors'
import { mountReviewSession } from './ui-runtime'

const settings: LiveAnalysisSettings = {
  searchSeconds: 2,
  lines: 3,
  threadsAuto: false,
  threads: 2,
  memoryMb: 64,
  moveFeedbackEnabled: false,
}

function fixtures(overrides: Partial<CachePort> = {}) {
  let line: (value: string) => void = () => {}
  const sent: string[] = []
  const acquired = vi.fn()
  const released = vi.fn()
  const save = vi.fn(() => Effect.succeed(1))
  const port: EnginePort = {
    send: (command) =>
      Effect.sync(() => {
        sent.push(command)
        if (command === 'uci') line('uciok')
        else if (command === 'isready') line('readyok')
        else if (command.startsWith('go ')) {
          line('info depth 20 multipv 1 score cp 42 pv e2e4')
          line('bestmove e2e4')
        }
      }),
    onLine(handler) {
      line = handler
      return () => {
        line = () => {}
      }
    },
  }
  const cache: CachePort = {
    get: () => Effect.succeed(null),
    getBulk: (fens) => Effect.succeed(fens.map(() => null)),
    put: () => Effect.void,
    putMany: () => Effect.void,
    ...overrides,
  }
  return {
    sent,
    acquired,
    released,
    save,
    layer: Layer.mergeAll(
      Layer.succeed(Engine, {
        acquire: Effect.acquireRelease(
          Effect.sync(() => {
            acquired()
            return port
          }),
          () =>
            Effect.sync(() => {
              released()
            }),
        ),
      }),
      Layer.succeed(PositionCache, cache),
      Layer.succeed(GamesRepository, { saveReview: save }),
      Layer.succeed(SystemResources, {
        get: Effect.succeed({ threads: 4, memory_mb: 8192 }),
      }),
    ),
  }
}

describe('native Effect session', () => {
  it('retains UI intent submitted before the runtime has finished starting', async () => {
    const f = fixtures()
    const store = createReviewStore()
    const result = existingResult()
    const mounted = mountReviewSession(
      {
        config: depthConfig({ initialResult: result }),
        store,
        onStateChange: () => {},
        onProgress: () => {},
      },
      f.layer,
    )
    const started = mounted.start()
    mounted.analyzePosition({ fen: result.positions[0].fen }, settings)
    try {
      await started
      await vi.waitFor(() =>
        expect(
          store.getState().liveAnalysis.positions[result.positions[0].fen],
        ).toBeDefined(),
      )
    } finally {
      await mounted.dispose()
    }
    expect(f.released).toHaveBeenCalledOnce()
  })

  it('coalesces a burst of navigation into the latest request', async () => {
    const f = fixtures()
    const store = createReviewStore()
    const result = existingResult()
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const session = yield* createReviewSession({
            config: depthConfig({ initialResult: result }),
            store,
            onStateChange: () => {},
            onProgress: () => {},
          })
          yield* session.start()
          for (let i = 0; i < 100; i++) {
            yield* session.analyzePosition(
              { fen: result.positions[i % 3].fen },
              settings,
            )
          }
          yield* TestClock.adjust(0)
          expect(
            store.getState().liveAnalysis.positions[result.positions[0].fen]
              ?.cp,
          ).toBe(42)
          expect(
            f.sent.filter((command) => command.startsWith('go ')),
          ).toHaveLength(1)
        }),
      ).pipe(Effect.provide(f.layer), Effect.provide(TestContext.TestContext)),
    )
    expect(f.acquired).toHaveBeenCalledOnce()
    expect(f.released).toHaveBeenCalledOnce()
  })

  it('interrupts a blocked cache lookup without waiting for its old result', async () => {
    const entered = Effect.runSync(Deferred.make<void>())
    const blocked = Effect.runSync(Deferred.make<null>())
    const result = existingResult()
    const [first, second] = result.positions
    const f = fixtures({
      get: (fen) =>
        fen === first.fen
          ? Effect.zipRight(
              Deferred.succeed(entered, undefined),
              Deferred.await(blocked),
            )
          : Effect.succeed(null),
    })
    const store = createReviewStore()
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const session = yield* createReviewSession({
            config: depthConfig({ initialResult: result }),
            store,
            onStateChange: () => {},
            onProgress: () => {},
          })
          yield* session.start()
          yield* session.analyzePosition({ fen: first.fen }, settings)
          yield* Deferred.await(entered)
          yield* session.analyzePosition({ fen: second.fen }, settings)
          yield* TestClock.adjust(0)
          expect(
            store.getState().liveAnalysis.positions[second.fen],
          ).toBeDefined()
          yield* Deferred.succeed(blocked, null)
          yield* TestClock.adjust(0)
          expect(
            store.getState().liveAnalysis.positions[first.fen],
          ).toBeUndefined()
          expect(f.sent).not.toContain(`position fen ${first.fen}`)
        }),
      ).pipe(Effect.provide(f.layer), Effect.provide(TestContext.TestContext)),
    )
  })

  it('starts once, supervises a failed save and retains the completed review', async () => {
    const f = fixtures()
    const store = createReviewStore()
    const states: ReviewSessionState[] = []
    const saved = vi.fn()
    const warnings = vi.fn()
    const failingGames = Layer.succeed(GamesRepository, {
      saveReview: () =>
        Effect.gen(function* () {
          saved()
          return yield* new PersistenceError({
            operation: 'save',
            message: 'disk full',
          })
        }),
    })
    const logger = Logger.replace(Logger.defaultLogger, Logger.make(warnings))
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const before = yield* diagnosticsSnapshot
          const session = yield* createReviewSession({
            config: depthConfig(),
            store,
            onStateChange: (state) => states.push(state),
            onProgress: () => {},
          })
          yield* session.start()
          yield* session.start()
          yield* TestClock.adjust(0)
          const after = yield* diagnosticsSnapshot
          expect(
            after.persistenceFailures.count - before.persistenceFailures.count,
          ).toBe(1)
          expect(saved).toHaveBeenCalledOnce()
          expect(warnings).toHaveBeenCalledOnce()
          expect(states).toEqual([{ status: 'done', error: null }])
          expect(store.getState().result?.moves).toHaveLength(2)
          expect(f.released).toHaveBeenCalledOnce()
        }),
      ).pipe(
        Effect.provide(Layer.merge(f.layer, failingGames)),
        Effect.provide(logger),
        Effect.provide(TestContext.TestContext),
      ),
    )
  })
})
