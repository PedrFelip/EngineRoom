import { Schema as S } from 'effect'

const Num = S.Number.pipe(S.check(S.isFinite()))
const Count = Num.pipe(S.check(S.isInt()), S.check(S.isGreaterThanOrEqualTo(0)))
const Positive = Count.pipe(S.check(S.isGreaterThan(0)))
const Strings = S.mutable(S.Array(S.String))
const Phase = S.Literals(['opening', 'middlegame', 'endgame'])
const Classification = S.Literals([
  'livro',
  'melhor',
  'excelente',
  'bom',
  'imprecisao',
  'erro',
  'blunder',
])
const Accuracy = S.Struct({ white: Num, black: Num })
export const PvLineSchema = S.Struct({
  multipv: Positive,
  san: S.NullOr(S.String),
  cp: Num,
  winPct: Num,
  pv: Strings,
  depth: S.optional(Count),
})
export const PositionAnalysisSchema = S.Struct({
  ply: Count,
  fen: S.String,
  phase: Phase,
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
})
export const ReviewResultSchema = S.Struct({
  positions: S.mutable(S.Array(PositionAnalysisSchema)),
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
        cpLoss: Num,
        bestUci: S.NullOr(S.String),
        isBook: S.Boolean,
        eco: S.NullOr(S.Struct({ code: S.String, name: S.String })),
      }),
    ),
  ),
  accuracyModel: S.String,
  accuracy: Accuracy,
  accuracyByPhase: S.Struct({
    opening: Accuracy,
    middlegame: Accuracy,
    endgame: Accuracy,
  }),
})
export const ReviewConfigSchema = S.Struct({
  pgn: S.String,
  meta: S.Struct({
    white: S.String,
    black: S.String,
    whiteElo: S.NullOr(S.String),
    blackElo: S.NullOr(S.String),
    result: S.String,
    event: S.NullOr(S.String),
    plies: Count,
  }),
  engine: S.Struct({
    id: S.Literals(['fast', 'balanced', 'deep', 'custom']),
    label: S.String,
    depth: Positive,
    hint: S.String,
  }),
  mode: S.Literals(['depth', 'time']),
  analysisKind: S.optional(S.Literals(['manual', 'auto-fast', 'auto-deep'])),
  movetimeMs: S.optional(Positive),
  lines: Positive,
  initialResult: S.optional(ReviewResultSchema),
})
export const SessionErrorSchema = S.Struct({
  code: S.String,
  operation: S.String,
  message: S.String,
})
const Progress = S.Struct({
  stage: S.Literals([
    'preparing',
    'analyzing',
    'triage',
    'refinement',
    'finalizing',
  ]),
  completed: Count,
  total: Count,
  currentPly: Count,
  phase: S.NullOr(Phase),
  cachedPositions: Count,
  enginePositions: Count,
  remainingBudgetMs: S.optional(Count),
  update: S.optional(S.Struct({ index: Count, winPct: Num })),
})
const Envelope = {
  sessionId: S.String,
  sequence: Positive,
  requestId: S.optional(Positive),
}
export const SessionEventSchema = S.Union([
  S.Struct({ ...Envelope, type: S.Literal('progress'), progress: Progress }),
  S.Struct({
    ...Envelope,
    type: S.Literal('completed'),
    result: ReviewResultSchema,
  }),
  S.Struct({
    ...Envelope,
    requestId: Positive,
    type: S.Literal('liveStarted'),
    fen: S.String,
  }),
  S.Struct({
    ...Envelope,
    requestId: Positive,
    type: S.Literal('liveCompleted'),
    fen: S.String,
    analysis: PositionAnalysisSchema,
  }),
  S.Struct({
    ...Envelope,
    requestId: Positive,
    type: S.Literal('classification'),
    nodeId: S.String,
    classification: Classification,
  }),
  S.Struct({
    ...Envelope,
    type: S.Literal('error'),
    error: SessionErrorSchema,
    fen: S.optional(S.String),
  }),
  S.Struct({
    ...Envelope,
    type: S.Literal('warning'),
    error: SessionErrorSchema,
  }),
])
export type SessionEvent = typeof SessionEventSchema.Type
export const ProbeResultSchema = S.Struct({
  ok: S.Boolean,
  name: S.NullOr(S.String),
  error: S.optional(S.String),
})

export type Classification = typeof Classification.Type
export type Phase = typeof Phase.Type
export type PvLine = typeof PvLineSchema.Type
export type PositionAnalysis = typeof PositionAnalysisSchema.Type
export type ReviewResult = typeof ReviewResultSchema.Type
