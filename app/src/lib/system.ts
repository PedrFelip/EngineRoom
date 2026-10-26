import { Effect } from 'effect'
import { errorMessage, SystemResourcesError } from './effect/errors'
import { decode, ipc } from './effect/ipc'
import { ResourcesSchema } from './effect/schemas'

/** Logical CPU cores + total system RAM (MB), used to size the Stockfish engine. */
export function getSystemResources() {
  return ipc(
    'system_resources',
    undefined,
    (cause) =>
      new SystemResourcesError({ cause, message: errorMessage(cause) }),
  ).pipe(
    Effect.flatMap((data) => decode(ResourcesSchema, data, 'system_resources')),
  )
}
