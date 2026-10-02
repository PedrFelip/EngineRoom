import { Effect, Schema } from 'effect'
import type { GameCursor } from '../types'
import { PersistenceError } from './effect/errors'
import { decode, ipc } from './effect/ipc'
import { GamePageSchema, StoredGameSchema } from './effect/schemas'
import { ReviewConfigSchema } from './review-protocol'

export function listGames(limit: number, cursor: GameCursor | null = null) {
  return gamesIpc('games_list', { limit, cursor }).pipe(
    Effect.flatMap((data) => decode(GamePageSchema, data, 'games_list')),
  )
}
export function getGame(id: number) {
  return gamesIpc('games_get', { id }).pipe(
    Effect.flatMap((data) =>
      decode(Schema.NullOr(StoredGameSchema), data, 'games_get'),
    ),
  )
}
export function getReviewConfig(id: number) {
  return gamesIpc('games_get_review_config', { id }).pipe(
    Effect.flatMap((data) =>
      decode(
        Schema.NullOr(ReviewConfigSchema),
        data,
        'games_get_review_config',
      ),
    ),
  )
}
export function deleteGame(id: number) {
  return gamesIpc('games_delete', { id }).pipe(Effect.asVoid)
}
export function clearGames() {
  return gamesIpc('games_clear').pipe(Effect.asVoid)
}
function gamesIpc(operation: string, args?: Record<string, unknown>) {
  return ipc(
    operation,
    args,
    (cause) =>
      new PersistenceError({
        operation,
        cause,
        message:
          typeof cause === 'object' && cause !== null && 'message' in cause
            ? String(cause.message)
            : String(cause),
      }),
  )
}
