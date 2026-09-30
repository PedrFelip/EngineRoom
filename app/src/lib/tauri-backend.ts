import { Channel } from '@tauri-apps/api/core'
import { Deferred, Effect, Either, Layer, Schema } from 'effect'
import { AnalysisSessions, type SessionPort } from './backend'
import { bestEffort } from './effect/diagnostics'
import { InvalidPayloadError, SessionError } from './effect/errors'
import { ipc } from './effect/ipc'
import { SessionErrorSchema, SessionEventSchema } from './review-protocol'

export function sessionCommand(
  command: string,
  args?: Record<string, unknown>,
) {
  return ipc(command, args, (cause) => {
    const parsed = Schema.decodeUnknownEither(SessionErrorSchema)(cause)
    return new SessionError(
      Either.isRight(parsed)
        ? parsed.right
        : {
            code: 'ipc',
            operation: command,
            message: cause instanceof Error ? cause.message : String(cause),
            cause,
          },
    )
  }).pipe(Effect.asVoid)
}

export const TauriBackend = Layer.succeed(AnalysisSessions, {
  open: (config, onEvent, onError) =>
    Effect.gen(function* () {
      const sessionId = crypto.randomUUID()
      let active = true
      let sequence = 0
      const invalid = yield* Deferred.make<void>()
      const channel = new Channel<unknown>()
      channel.onmessage = (payload) => {
        if (!active) return
        const parsed = Schema.decodeUnknownEither(SessionEventSchema)(payload)
        if (Either.isLeft(parsed)) {
          active = false
          onError(
            new InvalidPayloadError({
              source: 'review session',
              message: 'Dados inválidos da sessão de análise.',
            }),
          )
          Deferred.unsafeDone(invalid, Effect.void)
          return
        }
        const event = parsed.right
        if (event.sessionId !== sessionId || event.sequence <= sequence) return
        sequence = event.sequence
        onEvent(event)
      }
      yield* Effect.acquireRelease(
        sessionCommand('review_session_open', {
          sessionId,
          config,
          onEvent: channel,
        }),
        () =>
          Effect.gen(function* () {
            active = false
            yield* bestEffort(
              sessionCommand('review_session_close', { sessionId }),
              'session.close',
            )
          }),
      )
      yield* Deferred.await(invalid).pipe(
        Effect.zipRight(
          bestEffort(
            sessionCommand('review_session_close', { sessionId }),
            'session.invalid_payload',
          ),
        ),
        Effect.forkScoped,
      )
      return {
        analyzePosition: (requestId, request, settings) =>
          sessionCommand('review_session_analyze_position', {
            sessionId,
            requestId,
            request,
            settings,
          }),
        cancelLive: (requestId) =>
          sessionCommand('review_session_cancel_live', {
            sessionId,
            requestId,
          }),
      } satisfies SessionPort
    }),
})
