import { Deferred, Effect, Fiber } from 'effect'
import { TestClock } from 'effect/testing'
import { describe, expect, it } from 'vitest'
import { SessionError } from './errors'
import { runUi } from './ui-runtime'

describe('UI Effect boundary', () => {
  it('preserves domain errors and reports defects', async () => {
    const error = new SessionError({
      code: 'ipc',
      operation: 'open',
      message: 'unavailable',
    })
    await expect(runUi(Effect.fail(error))).rejects.toBe(error)
    await expect(runUi(Effect.die(new Error('unexpected')))).rejects.toThrow(
      'unexpected',
    )
  })

  it('acknowledges interruption only after scoped cleanup finishes', async () => {
    await runUi(
      Effect.gen(function* () {
        const acquired = yield* Deferred.make<void>()
        const releasing = yield* Deferred.make<void>()
        let released = false
        let acknowledged = false
        const owner = yield* Effect.scoped(
          Effect.gen(function* () {
            yield* Effect.acquireRelease(
              Deferred.succeed(acquired, undefined),
              () =>
                Effect.gen(function* () {
                  yield* Deferred.succeed(releasing, undefined)
                  yield* Effect.sleep('1 second')
                  released = true
                }),
            )
            yield* Effect.never
          }),
        ).pipe(Effect.forkChild)
        yield* Deferred.await(acquired)
        const closing = yield* Fiber.interrupt(owner).pipe(
          Effect.tap(() =>
            Effect.sync(() => {
              acknowledged = true
            }),
          ),
          Effect.forkChild,
        )
        yield* Deferred.await(releasing)
        yield* TestClock.adjust('999 millis')
        expect(released).toBe(false)
        expect(acknowledged).toBe(false)
        yield* TestClock.adjust('1 millis')
        yield* Fiber.join(closing)
        expect(released).toBe(true)
        expect(acknowledged).toBe(true)
      }).pipe(Effect.provide(TestClock.layer())),
    )
  })
})
