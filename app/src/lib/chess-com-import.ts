import { Channel } from '@tauri-apps/api/core'
import { Context, Data, Deferred, Effect, Either, Layer, Schema } from 'effect'
import { bestEffort } from './effect/diagnostics'
import { InvalidPayloadError } from './effect/errors'
import { ipc } from './effect/ipc'

const Count = Schema.Number.pipe(Schema.int(), Schema.nonNegative())
const Timestamp = Count.pipe(Schema.between(1, 253402300799))
const Text = Schema.String.pipe(Schema.minLength(1))
const ErrorPayloadSchema = Schema.Struct({
  code: Schema.Literal(
    'invalidInput',
    'unavailable',
    'rateLimited',
    'network',
    'timeout',
    'invalidResponse',
    'responseTooLarge',
    'cancelled',
  ),
  message: Text,
  retryAfterSeconds: Schema.NullOr(Count),
})
export class ChessComImportError extends Data.TaggedError(
  'ChessComImportError',
)<typeof ErrorPayloadSchema.Type> {}

const CursorSchema = Schema.Struct({
  username: Text,
  year: Count.pipe(Schema.between(2000, 9999)),
  month: Count.pipe(Schema.between(1, 12)),
  before: Schema.NullOr(Schema.Struct({ endTime: Timestamp, url: Text })),
})
const PlayerSchema = Schema.Struct({
  username: Text,
  rating: Schema.NullOr(Count),
})
const RemoteGameSchema = Schema.Struct({
  url: Text,
  endTime: Timestamp,
  timeClass: Schema.NullOr(Schema.String),
  white: PlayerSchema,
  black: PlayerSchema,
  result: Schema.Literal('1-0', '0-1', '1/2-1/2'),
  pgn: Text,
})
export const SearchResultSchema = Schema.Struct({
  items: Schema.Array(RemoteGameSchema).pipe(Schema.maxItems(20)),
  nextCursor: Schema.NullOr(CursorSchema),
  archivesRead: Count,
  skippedInvalid: Count,
  skippedUnsupported: Count,
  stopReason: Schema.Literal('exhausted', 'page', 'limit', 'error'),
  warning: Schema.NullOr(ErrorPayloadSchema),
})
const EventSchema = Schema.Union(
  Schema.Struct({
    requestId: Text,
    type: Schema.Literal('completed'),
    result: SearchResultSchema,
  }),
  Schema.Struct({
    requestId: Text,
    type: Schema.Literal('error'),
    error: ErrorPayloadSchema,
  }),
)
export type SearchResult = typeof SearchResultSchema.Type
export type RemoteGame = typeof RemoteGameSchema.Type
export type SearchCursor = typeof CursorSchema.Type
export type ImportFailure = ChessComImportError | InvalidPayloadError

export class ChessComSearch extends Context.Tag('EngineRoom/ChessComSearch')<
  ChessComSearch,
  {
    search(
      username: string,
      cursor: SearchCursor | null,
    ): Effect.Effect<SearchResult, ImportFailure>
  }
>() {}

function command(name: string, args: Record<string, unknown>) {
  return ipc(name, args, (cause) => {
    const parsed = Schema.decodeUnknownEither(ErrorPayloadSchema)(cause)
    return Either.isRight(parsed)
      ? new ChessComImportError(parsed.right)
      : new InvalidPayloadError({
          source: name,
          message: 'Não foi possível comunicar com a importação do Chess.com.',
          cause,
        })
  }).pipe(Effect.asVoid)
}

export const TauriChessComSearch = Layer.succeed(ChessComSearch, {
  search: (username, cursor) =>
    Effect.scoped(
      Effect.gen(function* () {
        const requestId = crypto.randomUUID()
        const result = yield* Deferred.make<SearchResult, ImportFailure>()
        const channel = new Channel<unknown>()
        let active = true
        channel.onmessage = (payload) => {
          if (!active) return
          if (
            typeof payload === 'object' &&
            payload !== null &&
            'requestId' in payload &&
            payload.requestId !== requestId
          )
            return
          const parsed = Schema.decodeUnknownEither(EventSchema)(payload)
          if (Either.isLeft(parsed)) {
            Deferred.unsafeDone(
              result,
              Effect.fail(
                new InvalidPayloadError({
                  source: 'Chess.com',
                  message:
                    'Dados inválidos recebidos da importação do Chess.com.',
                }),
              ),
            )
            return
          }
          const event = parsed.right
          Deferred.unsafeDone(
            result,
            event.type === 'completed'
              ? Effect.succeed(event.result)
              : event.error.code === 'cancelled'
                ? Effect.interrupt
                : Effect.fail(new ChessComImportError(event.error)),
          )
        }
        yield* Effect.acquireRelease(
          command('chess_com_search_start', {
            requestId,
            username,
            cursor,
            onEvent: channel,
          }),
          () => {
            active = false
            return bestEffort(
              command('chess_com_search_cancel', { requestId }),
              'chess_com.cancel',
            )
          },
        )
        return yield* Deferred.await(result)
      }),
    ),
})
