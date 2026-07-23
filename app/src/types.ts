export type EngineTierId = 'fast' | 'balanced' | 'deep' | 'custom'

/** Modo de análise: profundidade fixa ou tempo fixo por lance. */
export type EngineMode = 'depth' | 'time'

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
  /** Modo da análise: 'depth' usa `engine.depth`, 'time' usa `movetimeMs`. */
  mode: EngineMode
  /** Milissegundos por lance quando `mode === 'time'` (ignorado em 'depth'). */
  movetimeMs?: number
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
  mode: EngineMode
  /** Profundidade (mode='depth') ou milissegundos por lance (mode='time'). */
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

/**
 * Lance jogado pelo usuário numa linha alternativa durante a revisão. Nasce
 * "pendente" (sem afterCp/classification); o refino ao vivo preenche os campos
 * opcionais conforme a engine avalia a posição resultante.
 */
export interface VariationMove {
  id: string
  /** Índice (1-based) do lance dentro da sua variação. */
  ply: number
  color: 'w' | 'b'
  san: string
  uci: string
  fenBefore: string
  fenAfter: string
  /** cp cru (POV do lado a jogar) após o lance — serve de beforeCp do próximo. */
  afterCp?: number
  /** Linhas candidatas (multipv) normalizadas para o POV das brancas. */
  lines?: PvLine[]
  depth?: number
  bestUci?: string
  classification?: Classification
  winPctBefore?: number
  winPctAfter?: number
  winPctLoss?: number
}

/**
 * Sublinha jogada a partir de um ply da linha principal. `parentPly` é o índice
 * da posição (em `ReviewResult.positions`) de onde a variação ramifica.
 */
export interface Variation {
  id: string
  parentPly: number
  moves: VariationMove[]
}

/** Mapa de variações por ply-pai na linha principal. */
export type VariationMap = Record<number, Variation[]>
