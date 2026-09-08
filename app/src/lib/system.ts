import { Effect } from 'effect'
import { errorMessage, SystemResourcesError } from './effect/errors'
import { decode, ipc } from './effect/ipc'
import { ResourcesSchema } from './effect/schemas'

export interface SystemResources {
  threads: number
  memory_mb: number
}

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

/// Sizes the Stockfish hash table to ~20% of system RAM, clamped to a sane range
/// (512 MB floor, 4 GB ceiling). For fixed-depth per-position analysis this is the
/// sweet spot: enough to cache cross-position transpositions without starving the
/// OS/webview. Beyond ~4 GB the returns flatten.
export function recommendedHashMb(memoryMb: number): number {
  const hash = Math.floor(memoryMb * 0.2)
  return Math.min(Math.max(hash, 512), 4 * 1024)
}
