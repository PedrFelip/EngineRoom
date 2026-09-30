import {
  Cause,
  Deferred,
  Effect,
  Exit,
  Fiber,
  Option,
  TestClock,
  TestContext,
} from 'effect'
import { describe, expect, it, vi } from 'vitest'
import {
  ask,
  evalPosition,
} from '../__tests__/reference/analysis/engine-analysis'
import type {
  EngineExitReason,
  EnginePort,
} from '../__tests__/reference/analyze'
import {
  EngineCommandError,
  EngineExitedError,
  EngineTimeoutError,
} from './errors'

const FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

function controlledPort() {
  const lines = new Set<(line: string) => void>()
  const exits = new Set<(reason: EngineExitReason) => void>()
  const sent: string[] = []
  const offLine = vi.fn()
  const offExit = vi.fn()
  const port: EnginePort = {
    send: (command) =>
      Effect.sync(() => {
        sent.push(command)
      }),
    onLine(handler) {
      lines.add(handler)
      return () => {
        lines.delete(handler)
        offLine()
      }
    },
    onExit(handler) {
      exits.add(handler)
      return () => {
        exits.delete(handler)
        offExit()
      }
    },
  }
  return {
    port,
    sent,
    offLine,
    offExit,
    emit: (line: string) => {
      for (const handler of lines) handler(line)
    },
    exit: (reason: EngineExitReason) => {
      for (const handler of exits) handler(reason)
    },
  }
}

describe('Effect lifecycle with Vitest 4 and TestClock', () => {
  it('ignores lines arriving after bestmove, even before send resolves', () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const fake = controlledPort()
        fake.port.send = (command) =>
          Effect.sync(() => {
            if (!command.startsWith('go ')) return
            fake.emit('info depth 20 score cp 42 pv e2e4')
            fake.emit('bestmove e2e4')
            fake.emit('info depth 30 score cp 999 pv d2d4')
          })
        const result = yield* evalPosition(
          fake.port,
          FEN,
          { mode: 'depth', depth: 20 },
          1000,
        )
        expect(result.cp).toBe(42)
        expect(result.depth).toBe(20)
        expect(result.pv).toEqual(['e2e4'])
      }),
    ))

  it('uses a typed timeout and removes all subscriptions exactly once', () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const fake = controlledPort()
        const fiber = yield* ask(
          fake.port,
          'uci',
          (line) => line === 'uciok',
          1000,
        ).pipe(Effect.fork)
        yield* TestClock.adjust(1000)
        const result = yield* Fiber.await(fiber)
        expect(Exit.isFailure(result)).toBe(true)
        if (Exit.isFailure(result)) {
          expect(Cause.failureOption(result.cause)).toEqual(
            Option.some(
              new EngineTimeoutError({
                command: 'uci',
                timeoutMs: 1000,
                message: "A engine não respondeu a 'uci' em 1000ms.",
              }),
            ),
          )
        }
        expect(fake.sent).toEqual(['uci'])
        expect(fake.offLine).toHaveBeenCalledOnce()
        expect(fake.offExit).toHaveBeenCalledOnce()
      }).pipe(Effect.provide(TestContext.TestContext)),
    ))

  it('interruption stops a pending search and cannot publish a late result', () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const fake = controlledPort()
        const published = vi.fn()
        const ready = yield* Deferred.make<void>()
        fake.port.send = (command) =>
          Effect.gen(function* () {
            fake.sent.push(command)
            if (command.startsWith('go '))
              yield* Deferred.succeed(ready, undefined)
          })
        const fiber = yield* evalPosition(
          fake.port,
          FEN,
          { mode: 'depth', depth: 20 },
          1000,
        ).pipe(
          Effect.tap((position) => Effect.sync(() => published(position))),
          Effect.fork,
        )
        yield* Deferred.await(ready)
        const result = yield* Fiber.interrupt(fiber)
        expect(
          Exit.isFailure(result) && Cause.isInterruptedOnly(result.cause),
        ).toBe(true)
        fake.emit('info depth 30 score cp 999 pv e2e4')
        fake.emit('bestmove e2e4')
        expect(published).not.toHaveBeenCalled()
        expect(fake.sent).toContain('stop')
        expect(fake.offLine).toHaveBeenCalledOnce()
        expect(fake.offExit).toHaveBeenCalledOnce()
      }),
    ))

  it('engine exit fails immediately with structured exit information', () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const fake = controlledPort()
        fake.port.send = () =>
          Effect.sync(() => fake.exit({ code: null, signal: 11 }))
        const result = yield* Effect.flip(ask(fake.port, 'uci', () => false))
        expect(result).toBeInstanceOf(EngineExitedError)
        expect(result).toMatchObject({ signal: 11, code: null })
        expect(fake.offLine).toHaveBeenCalledOnce()
        expect(fake.offExit).toHaveBeenCalledOnce()
      }),
    ))

  it('command failure is retained when stopping the failed search also fails', () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const fake = controlledPort()
        fake.port.send = (command) => {
          if (command.startsWith('position')) return Effect.void
          return Effect.fail(
            new EngineCommandError({ command, message: command }),
          )
        }
        const error = yield* Effect.flip(
          evalPosition(fake.port, FEN, { mode: 'depth', depth: 20 }, 1000),
        )
        expect(error).toMatchObject({
          _tag: 'EngineCommandError',
          command: 'go depth 20',
        })
        expect(fake.offLine).toHaveBeenCalledOnce()
      }),
    ))
})
