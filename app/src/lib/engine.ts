import { Channel } from '@tauri-apps/api/core'
import { Deferred, Effect, Either, Schema } from 'effect'
import { bestEffort } from './effect/diagnostics'
import { ProbeResultSchema } from './review-protocol'
import { sessionCommand } from './tauri-backend'

export interface ProbeResult {
  ok: boolean
  name: string | null
  error?: string
}
export interface ProbeOptions {
  timeoutMs?: number
}

export function probeEngine({
  timeoutMs = 8000,
}: ProbeOptions = {}): Effect.Effect<ProbeResult> {
  return Effect.scoped(
    Effect.gen(function* () {
      const sessionId = crypto.randomUUID()
      const result = yield* Deferred.make<ProbeResult>()
      const channel = new Channel<unknown>()
      channel.onmessage = (payload) => {
        const parsed = Schema.decodeUnknownEither(ProbeResultSchema)(payload)
        Deferred.unsafeDone(
          result,
          Effect.succeed(
            Either.isRight(parsed)
              ? parsed.right
              : {
                  ok: false,
                  name: null,
                  error: 'Resposta inválida da engine.',
                },
          ),
        )
      }
      yield* Effect.acquireRelease(
        sessionCommand('engine_probe', {
          sessionId,
          onEvent: channel,
          timeoutMs,
        }),
        () =>
          bestEffort(
            sessionCommand('review_session_close', { sessionId }),
            'probe.close',
          ),
      )
      return yield* Deferred.await(result)
    }),
  ).pipe(
    Effect.catchAll((error) =>
      Effect.succeed({ ok: false, name: null, error: error.message }),
    ),
  )
}
