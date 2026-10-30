import { Effect, Schema as S } from 'effect'

const Num = S.Number.pipe(S.check(S.isFinite()))
const Count = Num.pipe(S.check(S.isInt()), S.check(S.isGreaterThanOrEqualTo(0)))
const Positive = Count.pipe(S.check(S.isGreaterThan(0)))
export const ResourcesSchema = S.Struct({
  threads: Positive,
  memory_mb: Positive,
})
export const StorageStatsSchema = S.Struct({
  cacheBytes: Count,
  gamesBytes: Count,
  dbBytes: Count,
})
const SummaryFields = {
  id: Positive,
  white: S.String,
  black: S.String,
  result: S.String,
  plies: Count,
  engineTier: S.String,
  mode: S.Literals(['depth', 'time']).pipe(
    S.withDecodingDefaultType(Effect.succeed('depth')),
  ),
  analysisKind: S.optional(S.Literals(['manual', 'auto-fast', 'auto-deep'])),
  depth: Count,
  multipv: Positive,
  accuracyWhite: Num,
  accuracyBlack: Num,
  createdAt: S.String,
}
export const StoredGameSchema = S.Struct({
  ...SummaryFields,
  pgn: S.String,
  reviewJson: S.String,
})
export const GamePageSchema = S.Struct({
  games: S.mutable(S.Array(S.Struct(SummaryFields))),
  total: Count,
  nextCursor: S.NullOr(S.Struct({ id: Positive, createdAt: S.String })),
})
// Keep field-level fallback/clamping: one invalid preference must not reset all others.
export const SettingsInputSchema = S.fromJsonString(
  S.Record(S.String, S.Unknown),
)
