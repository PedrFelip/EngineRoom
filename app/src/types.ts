export type EngineTierId = 'fast' | 'balanced' | 'deep'

export interface EngineTier {
  id: EngineTierId
  label: string
  depth: number
  hint: string
}

export const ENGINE_TIERS: EngineTier[] = [
  {
    id: 'fast',
    label: 'Rápido',
    depth: 15,
    hint: 'Pré-visualização rápida dos lances críticos.',
  },
  {
    id: 'balanced',
    label: 'Equilibrado',
    depth: 20,
    hint: 'Bom equilíbrio entre qualidade e tempo.',
  },
  {
    id: 'deep',
    label: 'Profundo',
    depth: 25,
    hint: 'Análise profunda, mais lenta por lance.',
  },
]

export interface PgnMeta {
  white: string
  black: string
  whiteElo: string | null
  blackElo: string | null
  result: string
  event: string | null
  plies: number
}

export interface ReviewConfig {
  pgn: string
  meta: PgnMeta
  engine: EngineTier
  lines: number
  /** Revisão já concluída (partida vinda do store) — pula a análise. */
  initialResult?: ReviewResult
}

/** Linha da lista de partidas analisadas (sem o peso do pgn/review). */
export interface GameSummary {
  id: number
  white: string
  black: string
  result: string
  plies: number
  engineTier: string
  depth: number
  multipv: number
  accuracyWhite: number
  accuracyBlack: number
  createdAt: string
}

/** Partida completa no store, para reabertura instantânea. */
export interface StoredGame extends GameSummary {
  pgn: string
  reviewJson: string
}

/** Classificação de um lance, no vocabulário pt-BR do projeto. */
export type Classification =
  | 'livro'
  | 'melhor'
  | 'excelente'
  | 'bom'
  | 'imprecisao'
  | 'erro'
  | 'blunder'

/** Resultado da análise do engine numa única posição. */
export interface PositionAnalysis {
  ply: number
  fen: string
  depth: number
  cp: number
  winPct: number
  pv: string[]
  lines: PvLine[]
}

/** Uma linha candidata (multipv) numa posição, normalizada p/ POV das brancas. */
export interface PvLine {
  multipv: number
  san: string | null
  cp: number
  winPct: number
  pv: string[]
}

/** Lance jogado com sua avaliação e classificação. */
export interface MoveAnalysis {
  ply: number
  color: 'w' | 'b'
  san: string
  uci: string
  fenBefore: string
  classification: Classification
  winPctBefore: number
  winPctAfter: number
  winPctLoss: number
  bestUci: string | null
  isBook: boolean
  eco: { code: string; name: string } | null
}

/** Precisão agregada (0–100) por cor. */
export interface AccuracyByColor {
  white: number
  black: number
}

/** Revisão completa de uma partida. */
export interface ReviewResult {
  positions: PositionAnalysis[]
  moves: MoveAnalysis[]
  accuracy: AccuracyByColor
}
