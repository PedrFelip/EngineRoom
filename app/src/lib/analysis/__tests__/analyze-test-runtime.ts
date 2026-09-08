/** Adapter for pre-migration regression fixtures. Production APIs are Effect-first;
 * retaining these fixtures lets the same inputs/assertions verify the new core. */
import { Effect, Logger } from 'effect'
import { vi } from 'vitest'
import * as Analysis from '../../analyze'
import {
  CacheError,
  EngineCommandError,
  errorMessage,
} from '../../effect/errors'
import { runUi } from '../../effect/ui-runtime'
import { evalPosition as evaluate } from '../engine-analysis'

export const testWarnings = vi.fn()
const testLogger = Logger.replace(
  Logger.defaultLogger,
  Logger.make((entry) => {
    if (entry.logLevel._tag === 'Warning') testWarnings(entry)
  }),
)

export * from '../../analyze'
export type EnginePort = Omit<Analysis.EnginePort, 'send' | 'sendBatch'> & {
  send(command: string): void | Promise<void>
  sendBatch?(commands: readonly string[]): void | Promise<void>
}
export type PositionCache = {
  [K in keyof Analysis.PositionCache]: (
    ...args: Parameters<Analysis.PositionCache[K]>
  ) => Promise<Effect.Effect.Success<ReturnType<Analysis.PositionCache[K]>>>
}
export function effectPort(port: EnginePort): Analysis.EnginePort {
  return {
    onLine: (handler) => port.onLine(handler),
    ...(port.onExit
      ? {
          onExit: (handler: Parameters<NonNullable<EnginePort['onExit']>>[0]) =>
            port.onExit?.(handler) ?? (() => {}),
        }
      : {}),
    send: (command) =>
      Effect.tryPromise({
        try: async () => {
          await port.send(command)
        },
        catch: (cause) =>
          new EngineCommandError({
            command,
            cause,
            message: errorMessage(cause),
          }),
      }),
    ...(port.sendBatch
      ? {
          sendBatch: (commands: readonly string[]) =>
            Effect.tryPromise({
              try: async () => {
                await port.sendBatch?.(commands)
              },
              catch: (cause) =>
                new EngineCommandError({
                  command: 'engine_send_batch',
                  cause,
                  message: errorMessage(cause),
                }),
            }),
        }
      : {}),
  }
}
export function effectCache(cache: PositionCache): Analysis.PositionCache {
  function call<A>(operation: string, run: () => Promise<A>) {
    return Effect.tryPromise({
      try: run,
      catch: (cause) =>
        new CacheError({ operation, cause, message: errorMessage(cause) }),
    })
  }
  return {
    get: (...args) => call('get', () => cache.get(...args)),
    put: (...args) => call('put', () => cache.put(...args)),
    getBulk: (...args) => call('getBulk', () => cache.getBulk(...args)),
    putMany: (...args) => call('putMany', () => cache.putMany(...args)),
  }
}
type Options = Omit<
  NonNullable<Parameters<typeof Analysis.analyzeGame>[4]>,
  'cache'
> & { cache?: PositionCache }
function effectOptions(opts: Options) {
  return { ...opts, cache: opts.cache ? effectCache(opts.cache) : undefined }
}
export function analyzeGame(
  pgn: string,
  control: Analysis.AnalyzeControl,
  port: EnginePort,
  multipv = 1,
  opts: Options = {},
) {
  return runUi(
    Analysis.analyzeGame(
      pgn,
      control,
      effectPort(port),
      multipv,
      effectOptions(opts),
    ).pipe(Effect.provide(testLogger)),
  )
}
export function analyzeGameAdaptive(
  pgn: string,
  profile: Parameters<typeof Analysis.analyzeGameAdaptive>[1],
  port: EnginePort,
  opts: Options = {},
) {
  return runUi(
    Analysis.analyzeGameAdaptive(
      pgn,
      profile,
      effectPort(port),
      effectOptions(opts),
    ),
  )
}
export function evalPosition(
  port: EnginePort,
  ...args: Tail<Parameters<typeof evaluate>>
) {
  return runUi(evaluate(effectPort(port), ...args))
}
export function configureEngine(
  port: EnginePort,
  opts: Parameters<typeof Analysis.configureEngine>[1],
) {
  return runUi(Analysis.configureEngine(effectPort(port), opts))
}
type Tail<T extends unknown[]> = T extends [unknown, ...infer Rest]
  ? Rest
  : never
