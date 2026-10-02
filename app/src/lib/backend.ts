import { Context, type Effect, type Scope } from 'effect'
import type { ReviewConfig } from '../types'
import type { InvalidPayloadError, SessionError } from './effect/errors'
import type { SessionEvent } from './review-protocol'
import type {
  LiveAnalysisRequest,
  LiveAnalysisSettings,
} from './review-session'

export interface SessionPort {
  analyzePosition(
    requestId: number,
    request: LiveAnalysisRequest,
    settings: LiveAnalysisSettings,
  ): Effect.Effect<void, SessionError>
  cancelLive(requestId: number): Effect.Effect<void, SessionError>
}

/** The UI injects a session, never a process, cache or analysis algorithm. */
export class AnalysisSessions extends Context.Tag(
  'EngineRoom/AnalysisSessions',
)<
  AnalysisSessions,
  {
    open(
      config: ReviewConfig,
      onEvent: (event: SessionEvent) => void,
      onError: (error: SessionError | InvalidPayloadError) => void,
    ): Effect.Effect<SessionPort, SessionError, Scope.Scope>
  }
>() {}

export type Backend = AnalysisSessions
