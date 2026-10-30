import { Channel } from '@tauri-apps/api/core'
import { Data, Effect, Schema } from 'effect'
import { errorMessage } from './effect/errors'
import { decode, ipc } from './effect/ipc'

const UpdateInfoSchema = Schema.Struct({
  currentVersion: Schema.String,
  availableVersion: Schema.NullOr(Schema.String),
  notes: Schema.NullOr(Schema.String),
  supported: Schema.Boolean,
})
export type UpdateInfo = typeof UpdateInfoSchema.Type

const UpdateProgressSchema = Schema.Union([
  Schema.Struct({
    status: Schema.Literal('downloading'),
    downloaded: Schema.Number,
    total: Schema.NullOr(Schema.Number),
  }),
  Schema.Struct({ status: Schema.Literal('installing') }),
  Schema.Struct({ status: Schema.Literal('restarting') }),
  Schema.Struct({ status: Schema.Literal('restartRequired') }),
])
export type UpdateProgress = typeof UpdateProgressSchema.Type

export class UpdateError extends Data.TaggedError('UpdateError')<{
  message: string
  cause?: unknown
}> {}

function updateError(cause: unknown): UpdateError {
  return new UpdateError({ message: errorMessage(cause), cause })
}

export function checkAppUpdate() {
  return ipc('app_update_check', undefined, updateError).pipe(
    Effect.flatMap((raw) => decode(UpdateInfoSchema, raw, 'app_update_check')),
  )
}

export function installAppUpdate(onProgress: (event: UpdateProgress) => void) {
  return Effect.suspend(() => {
    const channel = new Channel<unknown>()
    channel.onmessage = (raw) => {
      if (Schema.is(UpdateProgressSchema)(raw)) onProgress(raw)
    }
    return ipc('app_update_install', { onProgress: channel }, updateError).pipe(
      Effect.asVoid,
    )
  })
}

export function restartAfterFailedUpdate() {
  return ipc('app_update_restart', undefined, updateError).pipe(Effect.asVoid)
}

export function openUpdateReleasePage(version: string | null) {
  return ipc('app_update_open_release_page', { version }, updateError).pipe(
    Effect.asVoid,
  )
}
