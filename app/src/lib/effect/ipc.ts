import { invoke } from '@tauri-apps/api/core'
import { Effect, Schema } from 'effect'
import { InvalidPayloadError } from './errors'

/** IPC is not abortable: resource acquisition must await it before release. */
export function ipc<E>(
  command: string,
  args: Record<string, unknown> | undefined,
  onError: (cause: unknown) => E,
): Effect.Effect<unknown, E> {
  return Effect.tryPromise({
    try: () => invoke<unknown>(command, args),
    catch: onError,
  })
}

export function decode<A, I>(
  schema: Schema.Schema<A, I>,
  input: unknown,
  source: string,
) {
  return Schema.decodeUnknown(schema)(input).pipe(
    Effect.mapError(
      (cause) =>
        new InvalidPayloadError({
          source,
          cause,
          message: `Dados inválidos: ${source}.`,
        }),
    ),
  )
}
