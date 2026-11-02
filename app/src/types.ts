import enginePolicy from '../src-tauri/src/review/engine-policy.json'

export type EngineTierId = 'fast' | 'balanced' | 'deep' | 'custom'

/** Modo de análise: profundidade fixa ou tempo fixo por lance. */
export type EngineMode = 'depth' | 'time'

/** Estratégia da revisão: controles manuais ou orçamento adaptativo em duas passagens. */
export type AnalysisKind = 'manual' | 'auto-fast' | 'auto-deep'

export interface EngineTier {
  id: EngineTierId
  label: string
  depth: number
  hint: string
}

export const ENGINE_TIERS: EngineTier[] = enginePolicy.tiers as EngineTier[]

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
  /** Omitido em revisões antigas; equivale a `manual`. */
  analysisKind?: AnalysisKind
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
  /** Estratégia persistida; ausente em dados antigos equivale a `manual`. */
  analysisKind?: AnalysisKind
  /** Profundidade (mode='depth') ou milissegundos por lance (mode='time'). */
  depth: number
  multipv: number
  accuracyWhite: number
  accuracyBlack: number
  createdAt: string
}

/** Cursor estável para paginar o histórico na ordem mais recente primeiro. */
export interface GameCursor {
  createdAt: string
  id: number
}

// Live review shapes are derived from the validated Rust protocol. Historical
// reviews are normalized by Rust before they reach this boundary.
export type Classification = import('./lib/review-protocol').Classification
export type Phase = import('./lib/review-protocol').Phase
export type PositionAnalysis = import('./lib/review-protocol').PositionAnalysis
export type PvLine = import('./lib/review-protocol').PvLine
export type MoveAnalysis = ReviewResult['moves'][number]
export type AccuracyByColor = ReviewResult['accuracy']
export type ReviewResult = import('./lib/review-protocol').ReviewResult
