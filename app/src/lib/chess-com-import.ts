import { Channel } from '@tauri-apps/api/core'
import { Context, Data, Deferred, Effect, Exit, Layer, Schema } from 'effect'
import { bestEffort } from './effect/diagnostics'
import { InvalidPayloadError } from './effect/errors'
import { ipc } from './effect/ipc'

const Count = Schema.Number.pipe(
  Schema.check(Schema.isInt()),
  Schema.check(Schema.isGreaterThanOrEqualTo(0)),
)
const Timestamp = Count.pipe(
  Schema.check(Schema.isBetween({ minimum: 1, maximum: 253402300799 })),
)
const Text = Schema.String.pipe(Schema.check(Schema.isMinLength(1)))
const ErrorPayloadSchema = Schema.Struct({
  code: Schema.Literals([
    'invalidInput',
    'unavailable',
    'rateLimited',
    'network',
    'timeout',
    'invalidResponse',
    'responseTooLarge',
    'cancelled',
  ]),
  message: Text,
  retryAfterSeconds: Schema.NullOr(Count),
})
export class ChessComImportError extends Data.TaggedError(
  'ChessComImportError',
)<typeof ErrorPayloadSchema.Type> {}

const CursorSchema = Schema.Struct({
  username: Text,
  year: Count.pipe(
    Schema.check(Schema.isBetween({ minimum: 2000, maximum: 9999 })),
  ),
  month: Count.pipe(
    Schema.check(Schema.isBetween({ minimum: 1, maximum: 12 })),
  ),
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
  result: Schema.Literals(['1-0', '0-1', '1/2-1/2']),
  pgn: Text,
})
export const SearchResultSchema = Schema.Struct({
  items: Schema.Array(RemoteGameSchema).pipe(
    Schema.check(Schema.isMaxLength(20)),
  ),
  nextCursor: Schema.NullOr(CursorSchema),
  archivesRead: Count,
  skippedInvalid: Count,
  skippedUnsupported: Count,
  stopReason: Schema.Literals(['exhausted', 'page', 'limit', 'error']),
  warning: Schema.NullOr(ErrorPayloadSchema),
})
const EventSchema = Schema.Union([
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
])
export type SearchResult = typeof SearchResultSchema.Type
export type RemoteGame = typeof RemoteGameSchema.Type
export type SearchCursor = typeof CursorSchema.Type
export type ImportFailure = ChessComImportError | InvalidPayloadError

export class ChessComSearch extends Context.Service<
  ChessComSearch,
  {
    search(
      username: string,
      cursor: SearchCursor | null,
    ): Effect.Effect<SearchResult, ImportFailure>
  }
>()('EngineRoom/ChessComSearch') {}

function command(name: string, args: Record<string, unknown>) {
  return ipc(name, args, (cause) => {
    const parsed = Schema.decodeUnknownExit(ErrorPayloadSchema)(cause)
    return Exit.isSuccess(parsed)
      ? new ChessComImportError(parsed.value)
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
          const parsed = Schema.decodeUnknownExit(EventSchema)(payload)
          if (Exit.isFailure(parsed)) {
            Deferred.doneUnsafe(
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
          const event = parsed.value
          Deferred.doneUnsafe(
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
