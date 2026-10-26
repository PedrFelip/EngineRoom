import { Data } from 'effect'

export class PersistenceError extends Data.TaggedError('PersistenceError')<{
  message: string
  operation: string
  cause?: unknown
}> {}
export class InvalidPayloadError extends Data.TaggedError(
  'InvalidPayloadError',
)<{
  message: string
  source: string
  cause?: unknown
}> {}
export class SystemResourcesError extends Data.TaggedError(
  'SystemResourcesError',
)<{
  message: string
  cause?: unknown
}> {}

export class SessionError extends Data.TaggedError('SessionError')<{
  code: string
  operation: string
  message: string
  cause?: unknown
}> {}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
