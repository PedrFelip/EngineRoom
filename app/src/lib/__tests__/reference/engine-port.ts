import { listen } from '@tauri-apps/api/event'
import { Effect, Either, Schema } from 'effect'
import { bestEffort } from '../../effect/diagnostics'
import {
  EngineCommandError,
  EngineSpawnError,
  errorMessage,
} from '../../effect/errors'
import { ipc } from '../../effect/ipc'
import { EngineExitSchema } from '../../effect/schemas'
import type { EngineExitReason, EnginePort } from './analyze'

export const ENGINE_LINE_EVENT = 'engine://line'
export const ENGINE_EXIT_EVENT = 'engine://exit'

// Shared by every production Layer, including Settings. An owner must finish
// cleanup before another can spawn; never stop a process owned by someone else.
const enginePermit = Effect.unsafeMakeSemaphore(1)

export function engineCommand(command: string, args?: Record<string, unknown>) {
  return ipc(
    command,
    args,
    (cause) =>
      new EngineCommandError({
        command,
        cause,
        message: errorMessage(cause),
      }),
  ).pipe(Effect.asVoid)
}

/** invoke/listen cannot abort: finish each acquisition and register its release
 * before honoring interruption. Each partial acquisition has its own finalizer. */
export const createTauriEnginePort = Effect.gen(function* () {
  yield* Effect.acquireRelease(Effect.interruptible(enginePermit.take(1)), () =>
    enginePermit.release(1),
  )
  const lines = new Set<(line: string) => void>()
  const exits = new Set<(reason: EngineExitReason) => void>()
  let lastExit: EngineExitReason | undefined
  const reportExit = (reason: EngineExitReason) => {
    lastExit = reason
    for (const handler of exits) handler(reason)
  }
  const subscribe = (event: string, handler: (payload: unknown) => void) =>
    Effect.acquireRelease(
      Effect.tryPromise({
        try: () => listen<unknown>(event, (e) => handler(e.payload)),
        catch: (cause) =>
          new EngineSpawnError({ cause, message: errorMessage(cause) }),
      }),
      (off) => Effect.sync(off),
    )
  yield* subscribe(ENGINE_LINE_EVENT, (payload) => {
    if (typeof payload === 'string')
      for (const handler of lines) handler(payload)
    else reportExit({ code: null, signal: null, error: 'Linha UCI inválida.' })
  })
  yield* subscribe(ENGINE_EXIT_EVENT, (payload) => {
    const decoded = Schema.decodeUnknownEither(EngineExitSchema)(payload)
    const reason = Either.isRight(decoded)
      ? decoded.right
      : {
          code: null,
          signal: null,
          error: 'Evento de saída inválido.',
        }
    reportExit(reason)
  })
  yield* Effect.acquireRelease(
    ipc(
      'engine_spawn',
      undefined,
      (cause) => new EngineSpawnError({ cause, message: errorMessage(cause) }),
    ),
    () => bestEffort(engineCommand('engine_stop'), 'engine.stop'),
  )
  return {
    send: (line) => engineCommand('engine_send', { line }),
    sendBatch: (lines) => engineCommand('engine_send_batch', { lines }),
    onLine(handler) {
      lines.add(handler)
      return () => {
        lines.delete(handler)
      }
    },
    onExit(handler) {
      exits.add(handler)
      if (lastExit) handler(lastExit)
      return () => {
        exits.delete(handler)
      }
    },
  } satisfies EnginePort
})
