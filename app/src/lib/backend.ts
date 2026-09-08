import { Context, type Effect, type Scope } from 'effect'
import type { ReviewConfig, ReviewResult } from '../types'
import type { PositionCache as CachePort, EnginePort } from './analyze'
import type {
  EngineError,
  InvalidPayloadError,
  PersistenceError,
  SystemResourcesError,
} from './effect/errors'
import type { SystemResources as Resources } from './system'

/** Services describe capabilities; production and fake Layers supply them. */
export class Engine extends Context.Tag('EngineRoom/Engine')<
  Engine,
  {
    readonly acquire: Effect.Effect<EnginePort, EngineError, Scope.Scope>
  }
>() {}
export class PositionCache extends Context.Tag('EngineRoom/PositionCache')<
  PositionCache,
  CachePort
>() {}
export class GamesRepository extends Context.Tag('EngineRoom/GamesRepository')<
  GamesRepository,
  {
    saveReview(
      config: ReviewConfig,
      result: ReviewResult,
    ): Effect.Effect<number, PersistenceError | InvalidPayloadError>
  }
>() {}
export class SystemResources extends Context.Tag('EngineRoom/SystemResources')<
  SystemResources,
  {
    readonly get: Effect.Effect<
      Resources,
      SystemResourcesError | InvalidPayloadError
    >
  }
>() {}
export type Backend = Engine | PositionCache | GamesRepository | SystemResources
