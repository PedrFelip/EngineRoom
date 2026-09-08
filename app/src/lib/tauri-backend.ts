import { Layer } from 'effect'
import {
  Engine,
  GamesRepository,
  PositionCache,
  SystemResources,
} from './backend'
import { createTauriPositionCache } from './cache'
import { createTauriEnginePort } from './engine-port'
import { saveReview } from './games'
import { getSystemResources } from './system'

export const TauriBackend = Layer.mergeAll(
  Layer.succeed(Engine, { acquire: createTauriEnginePort }),
  Layer.succeed(PositionCache, createTauriPositionCache()),
  Layer.succeed(GamesRepository, { saveReview }),
  Layer.succeed(SystemResources, { get: getSystemResources() }),
)
