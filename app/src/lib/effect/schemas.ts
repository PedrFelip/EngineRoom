import { Effect, Option, Predicate, Schema as S, SchemaGetter } from 'effect'
import { PvLineSchema } from '../review-protocol'

const Num = S.Number.pipe(S.check(S.isFinite()))
const Count = Num.pipe(S.check(S.isInt()), S.check(S.isGreaterThanOrEqualTo(0)))
const Positive = Count.pipe(S.check(S.isGreaterThan(0)))
const Strings = S.mutable(S.Array(S.String))
const Phase = S.Literals(['opening', 'middlegame', 'endgame'])
const Accuracy = S.Struct({ white: Num, black: Num })
const AccuracyByPhase = S.Struct({
  opening: Accuracy,
  middlegame: Accuracy,
  endgame: Accuracy,
})
const Classification = S.Literals([
  'livro',
  'melhor',
  'excelente',
  'bom',
  'imprecisao',
  'erro',
  'blunder',
  'brilhante',
  'otimo',
])

export const RawLineSchema = S.Struct({
  multipv: Positive,
  cp: Num,
  pv: Strings,
  san: S.optional(S.NullOr(S.String)),
  depth: S.optional(Count),
})
export const CachedPositionSchema = S.Struct({
  cp: Num,
  linesJson: S.String,
  reachedDepth: Count.pipe(S.withDecodingDefaultType(Effect.succeed(0))),
})
export const CacheLinesSchema = S.fromJsonString(
  S.mutable(S.Array(RawLineSchema)),
)
export const ResourcesSchema = S.Struct({
  threads: Positive,
  memory_mb: Positive,
})
export const StorageStatsSchema = S.Struct({
  cacheBytes: Count,
  gamesBytes: Count,
  dbBytes: Count,
})
export const EngineExitSchema = S.Struct({
  code: S.NullOr(S.Int),
  signal: S.NullOr(S.Int),
  error: S.optional(S.NullOr(S.String)).pipe(
    S.decodeTo(S.optional(S.String), {
      decode: SchemaGetter.transformOptional(
        Option.filter(Predicate.isNotNull),
      ),
      encode: SchemaGetter.passthrough(),
    }),
  ),
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
export const SavedIdSchema = Positive
export const LegacyReviewSchema = S.fromJsonString(
  S.Struct({
    positions: S.mutable(
      S.Array(
        S.Struct({
          ply: Count,
          fen: S.String,
          phase: S.optional(Phase),
          depth: Count,
          cp: Num,
          winPct: Num,
          pv: Strings,
          lines: S.mutable(S.Array(PvLineSchema)),
          triageLines: S.optional(S.mutable(S.Array(PvLineSchema))),
          search: S.optional(
            S.Struct({
              purpose: S.Literals(['playback', 'refinement']),
              movetimeMs: Count,
              multipv: Positive,
            }),
          ),
        }),
      ),
    ),
    moves: S.mutable(
      S.Array(
        S.Struct({
          ply: Positive,
          color: S.Literals(['w', 'b']),
          san: S.String,
          uci: S.String,
          fenBefore: S.String,
          classification: Classification,
          winPctBefore: Num,
          winPctAfter: Num,
          winPctLoss: Num,
          cpLoss: S.optional(Num),
          bestUci: S.NullOr(S.String),
          isBook: S.Boolean,
          eco: S.NullOr(S.Struct({ code: S.String, name: S.String })),
        }),
      ),
    ),
    accuracyModel: S.optional(S.String),
    accuracy: Accuracy,
    accuracyByPhase: S.optional(AccuracyByPhase),
  }),
)
export type LegacyReview = typeof LegacyReviewSchema.Type

// Keep field-level fallback/clamping: one invalid preference must not reset all others.
export const SettingsInputSchema = S.fromJsonString(
  S.Record(S.String, S.Unknown),
)
