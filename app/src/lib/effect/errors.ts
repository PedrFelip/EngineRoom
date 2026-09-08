import { Data } from 'effect'

export class EngineSpawnError extends Data.TaggedError('EngineSpawnError')<{
  message: string
  cause?: unknown
}> {}
export class EngineCommandError extends Data.TaggedError('EngineCommandError')<{
  message: string
  command: string
  cause?: unknown
}> {}
export class EngineTimeoutError extends Data.TaggedError('EngineTimeoutError')<{
  message: string
  command: string
  timeoutMs: number
}> {}
export class EngineExitedError extends Data.TaggedError('EngineExitedError')<{
  message: string
  code: number | null
  signal: number | null
}> {}
export class MissingEvaluationError extends Data.TaggedError(
  'MissingEvaluationError',
) {
  readonly message = 'A engine encerrou a busca sem avaliação da posição.'
}
export class CacheError extends Data.TaggedError('CacheError')<{
  message: string
  operation: string
  cause?: unknown
}> {}
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

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export type EngineError =
  | EngineSpawnError
  | EngineCommandError
  | EngineTimeoutError
  | EngineExitedError
  | MissingEvaluationError
  | InvalidPayloadError
export type AnalysisError = EngineError | CacheError
