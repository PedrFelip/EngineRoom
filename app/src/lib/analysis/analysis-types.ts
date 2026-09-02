import type { Phase } from '../../types'
import type { EcoEntry } from '../eco'

export interface BookInfo {
  maxPly: number
  eco: EcoEntry | null
}

export interface RawLine {
  multipv: number
  cp: number
  pv: string[]
  san?: string | null
  /** Profundidade (plies) que esta linha atingiu na busca. */
  depth?: number
}

export interface RawPosition {
  fen: string
  cp: number
  depth: number
  pv: string[]
  lines?: RawLine[]
}

export interface PlayedMove {
  ply: number
  color: 'w' | 'b'
  san: string
  uci: string
  fenBefore: string
}

export interface PlayedGame {
  startFen: string
  moves: PlayedMove[]
}

/** Motivo pelo qual o processo da engine encerrou (evento `engine://exit`). */
export interface EngineExitReason {
  /** Código de saída, quando conhecido (saída limpa ou sinal com código). */
  code: number | null
  /** Número do sinal que matou o processo, se houver (ex.: 11 = SIGSEGV). */
  signal: number | null
  /** Erro reportado pelo plugin de shell (falha de UTF-8/IO), se for o caso. */
  error?: string
}

/**
 * Interface do engine injetável, para testar com engine falso.
 * `onExit` é opcional: quando presente, `ask()` rejeita imediatamente se a
 * engine morrer, em vez de esperar o timeout completo.
 */
export interface EnginePort {
  send(cmd: string): void | Promise<void>
  onLine(handler: (line: string) => void): () => void
  onExit?(handler: (reason: EngineExitReason) => void): () => void
}

/**
 * Como a engine deve buscar cada posição:
 *  - `depth`: fixa em N ply (`go depth N`);
 *  - `time`: fixa em N ms (`go movetime N`), estilo chess.com "Maximum Time".
 */
export type AnalyzeControl =
  | { mode: 'depth'; depth: number }
  | { mode: 'time'; movetimeMs: number }

/**
 * Valor escalar usado como chave de cache: o `depth` no modo profundidade,
 * ou `movetimeMs` no modo tempo. A diferenciação entre modos é feita pelo
 * campo `mode` (também parte da chave), evitando colisão entre
 * `depth=20` e `movetimeMs=20`, por exemplo.
 */
export function controlKeyValue(control: AnalyzeControl): number {
  return control.mode === 'depth' ? control.depth : control.movetimeMs
}

/**
 * Orçamento padrão de timeout por posição para o comando `go`:
 *  - `depth`: busca sem limite inerente — 180s como rede de segurança contra
 *    engine travada (em hardware modesto, d25 pode chegar perto disso);
 *  - `time`: a engine se auto-limita a `movetimeMs`, então 3·N + 10s cobre
 *    folga de IPC/spawn.
 */
export function defaultGoTimeout(control: AnalyzeControl): number {
  return control.mode === 'depth' ? 180_000 : 3 * control.movetimeMs + 10_000
}

/** Modo de análise: por profundidade fixa ou por tempo fixo por lance. */
export type EngineMode = AnalyzeControl['mode']

export type AnalysisProgressStage =
  | 'analyzing'
  | 'triage'
  | 'refinement'
  | 'finalizing'

/** Progresso rico para a UI; `onProgress` permanece como seam do grafico. */
export interface AnalysisProgress {
  stage: AnalysisProgressStage
  completed: number
  total: number
  currentPly: number
  phase: Phase
  winPcts: number[]
  cachedPositions: number
  enginePositions: number
  /** Teto de tempo ainda orcado pelo modo `movetime`; ausente em depth. */
  remainingBudgetMs?: number
}

/**
 * Cache de avaliações por posição, chaveado por (fen, mode, value, multipv),
 * onde `value` é `depth` (modo profundidade) ou `movetimeMs` (modo tempo).
 * `get` devolve null em caso de miss; `put` grava a avaliação alcançada.
 *
 * `getBulk`/`putMany` são os equivalentes em lote para uma mesma chave
 * (mode, value, multipv): o pipeline de análise faz uma única consulta de
 * prefetch e um único descarrego ao final, em vez de N chamadas seriais.
 */
export interface PositionCache {
  get(
    fen: string,
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<RawPosition | null>
  put(
    pos: RawPosition,
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<void>
  /** Prefetch dos hits para N fens, numa única chamada. Ordem preservada. */
  getBulk(
    fens: string[],
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<(RawPosition | null)[]>
  /** Grava N posições numa única chamada (transação). */
  putMany(
    entries: RawPosition[],
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<void>
}
