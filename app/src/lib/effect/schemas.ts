import { Schema as S } from 'effect'

const Num = S.Number.pipe(S.finite())
const Count = Num.pipe(S.int(), S.nonNegative())
const Positive = Count.pipe(S.positive())
const Strings = S.mutable(S.Array(S.String))
const Phase = S.Literal('opening', 'middlegame', 'endgame')
const Accuracy = S.Struct({ white: Num, black: Num })
const AccuracyByPhase = S.Struct({
  opening: Accuracy,
  middlegame: Accuracy,
  endgame: Accuracy,
})
const Classification = S.Literal(
  'livro',
  'melhor',
  'excelente',
  'bom',
  'imprecisao',
  'erro',
  'blunder',
  'brilhante',
  'otimo',
)

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
  reachedDepth: S.optionalWith(Count, { default: () => 0 }),
})
export const CacheLinesSchema = S.parseJson(S.mutable(S.Array(RawLineSchema)))
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
  error: S.optionalWith(S.String, { nullable: true }),
})
const SummaryFields = {
  id: Positive,
  white: S.String,
  black: S.String,
  result: S.String,
  plies: Count,
  engineTier: S.String,
  mode: S.optionalWith(S.Literal('depth', 'time'), {
    default: () => 'depth' as const,
  }),
  analysisKind: S.optional(S.Literal('manual', 'auto-fast', 'auto-deep')),
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
export const LegacyReviewSchema = S.parseJson(
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
          lines: S.mutable(
            S.Array(
              S.Struct({
                multipv: Positive,
                san: S.NullOr(S.String),
                cp: Num,
                winPct: Num,
                pv: Strings,
              }),
            ),
          ),
          search: S.optional(
            S.Struct({
              purpose: S.Literal('playback', 'refinement'),
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
          color: S.Literal('w', 'b'),
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
export type LegacyReview = S.Schema.Type<typeof LegacyReviewSchema>

// Keep field-level fallback/clamping: one invalid preference must not reset all others.
export const SettingsInputSchema = S.parseJson(
  S.Record({ key: S.String, value: S.Unknown }),
)
