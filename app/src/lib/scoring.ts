/**
 * Conversão de avaliação (centipawns, POV do lado a jogar) em:
 *  - probabilidade de vitória (win%) via curva logística;
 *  - classificação de lances (Brilhante/Melhor/Excelente/Bom/Imprecisão/Erro/
 *    Blunder/Livro);
 *  - precisão agregada da partida (0–100%).
 *
 * Modelo Lichess: thresholds sobre delta de win% e fórmula de acurácia
 * 103.1668·exp(-0.04354·loss) - 3 aplicada sobre a média das perdas.
 * Mantido puro e sem efeitos colaterais.
 */

import type { Classification } from '../types'

export type { Classification }

/** Inclinação da curva logística cp→win% (constante do modelo Lichess). */
const WINPCT_K = 0.00368208

/** Rótulos em pt-BR exibidos na UI (badges, resumo). */
export const CLASSIFICATION_LABELS: Record<Classification, string> = {
  brilhante: 'Brilhante',
  livro: 'Livro',
  melhor: 'Melhor',
  excelente: 'Excelente',
  bom: 'Bom',
  imprecisao: 'Imprecisão',
  erro: 'Erro',
  blunder: 'Blunder',
}

/**
 * Converte centipawns (POV do lado a jogar) em probabilidade de vitória (0–100%).
 * Curva logística centrada em 50% para cp = 0.
 */
export function cpToWinPct(cp: number): number {
  return 50 + 50 * (2 / (1 + Math.exp(-WINPCT_K * cp)) - 1)
}

/**
 * Normaliza centipawns crus (POV do lado a jogar) para o POV das brancas.
 * Inverte o sinal quando as pretas estão a jogar. Único lugar que decide isso.
 */
export function whiteCp(cp: number, stm: 'w' | 'b'): number {
  return stm === 'w' ? cp : -cp
}

/**
 * win% (POV brancas) a partir de um cp cru (POV do lado a jogar) e do lado a
 * jogar. Inverte o espelho da curva logística quando as pretas jogam.
 */
export function whiteWinPct(cp: number, stm: 'w' | 'b'): number {
  return stm === 'w' ? cpToWinPct(cp) : 100 - cpToWinPct(cp)
}

/**
 * Cor do lado a jogar em `moves[ply]` (posição após `ply` lances): a cor do
 * próximo lance, ou o oposto do último lance na posição final. Lista vazia
 * devolve 'w' (posição inicial). Aceita qualquer lista com `color: 'w' | 'b'`.
 */
export function sideToMoveAtPly(
  moves: { color: 'w' | 'b' }[],
  ply: number,
): 'w' | 'b' {
  if (moves.length === 0) return 'w'
  if (ply < moves.length) return moves[ply].color
  const last = moves[moves.length - 1].color
  return last === 'w' ? 'b' : 'w'
}

/** Limiar (em delta de win%) abaixo do qual o lance é Excelente. */
const EXCELLENT_MAX_LOSS = 2
/** Limiar (em delta de win%) abaixo do qual o lance é Bom. */
const GOOD_MAX_LOSS = 5
/** Limiar (em delta de win%) abaixo do qual o lance é Imprecisão. */
const INACCURACY_MAX_LOSS = 10
/** Limiar (em delta de win%) abaixo do qual o lance é Erro. */
const MISTAKE_MAX_LOSS = 20

/**
 * Classifica um lance pela perda de win% em relação ao melhor lance (delta de win%).
 * Acima de MISTAKE_MAX_LOSS o lance é Blunder. Lance de abertura (isBook) é Livro.
 */
export function classifyMove(
  winPctLoss: number,
  isBook = false,
): Classification {
  if (isBook) return 'livro'
  if (winPctLoss <= 0) return 'melhor'
  if (winPctLoss <= EXCELLENT_MAX_LOSS) return 'excelente'
  if (winPctLoss <= GOOD_MAX_LOSS) return 'bom'
  if (winPctLoss <= INACCURACY_MAX_LOSS) return 'imprecisao'
  if (winPctLoss <= MISTAKE_MAX_LOSS) return 'erro'
  return 'blunder'
}

/**
 * Regras do lance Brilhante (estilo chess.com): o melhor lance da posição —
 * ou quase isso, quando a 2ª linha candidata confirma — que además sacrifica
 * material, sem deixar quem jogou em posição ruim nem partir de vitória
 * esmagadora. Quando aplicável, substitui a classificação corrente.
 */

/** Sacrifício mínimo (em peões, POV de quem jogou) para o lance ser Brilhante. */
export const BRILLIANT_MIN_SACRIFICE = 2
/** win% mínimo (POV de quem jogou) após o lance — não pode ficar em posição ruim. */
export const BRILLIANT_MIN_WINPCT_AFTER = 35
/** win% máximo (POV de quem jogou) antes do lance — não pode já estar ganhando. */
export const BRILLIANT_MAX_WINPCT_BEFORE = 85
/** Perda de win% tolerada com 2ª linha candidata; sem ela, exige o lance exato. */
const BRILLIANT_MAX_LOSS_WITH_2ND_LINE = 0.5

export interface BrilliantInput {
  /** Perda de win% do lance jogado vs o melhor (POV de quem jogou). */
  winPctLoss: number
  /** win% antes do lance, POV de quem jogou. */
  winPctBefore: number
  /** win% depois do lance, POV de quem jogou. */
  winPctAfter: number
  /**
   * Δ material de quem jogou (em peões) após o lance + a melhor resposta do
   * oponente. Negativo = entregou material (sacrifício).
   */
  materialDelta: number
  /** true quando há 2ª linha candidata (multipv ≥ 2) confirmando o "quase melhor". */
  hasSecondLine: boolean
}

/**
 * Verifica se um lance é Brilhante. Melhor/quase melhor + sacrifício bom,
 * sem ficar mal depois nem partir de posição já ganha. Puro.
 */
export function detectBrilliant(input: BrilliantInput): boolean {
  if (input.materialDelta > -BRILLIANT_MIN_SACRIFICE) return false
  if (input.winPctAfter < BRILLIANT_MIN_WINPCT_AFTER) return false
  if (input.winPctBefore > BRILLIANT_MAX_WINPCT_BEFORE) return false
  const maxLoss = input.hasSecondLine ? BRILLIANT_MAX_LOSS_WITH_2ND_LINE : 0
  return input.winPctLoss <= maxLoss
}

/** Constantes da fórmula de acurácia (modelo Lichess). */
const ACCURACY_CEIL = 103.1668
const ACCURACY_DECAY = 0.04354

/**
 * Precisão agregada da partida (0–100%) a partir das perdas de win% por lance.
 * Aplica a fórmula do Lichess (103.1668·exp(-0.04354·loss) - 3) sobre a média
 * das perdas, não por lance — evita o viés convexo de transformar-então-média.
 */
export function gameAccuracy(winPctLosses: number[]): number {
  if (winPctLosses.length === 0) return 100
  const meanLoss =
    winPctLosses.reduce((acc, loss) => acc + loss, 0) / winPctLosses.length
  return Math.min(100, ACCURACY_CEIL * Math.exp(-ACCURACY_DECAY * meanLoss) - 3)
}

/** Limiar acima do qual um cp é considerado xeque-mate (mate-in-N mapeado por scoreToCp). */
const MATE_CP = 90000

/**
 * Formata centipawns (POV das brancas) como string de avaliação: "+1.20" ou
 * "#3" / "-#3" para mate. Usado na barra de candidatas.
 */
export function formatEval(cp: number): string {
  if (cp >= MATE_CP) return `#${100000 - cp}`
  if (cp <= -MATE_CP) return `-#${100000 + cp}`
  const pawns = cp / 100
  return `${pawns >= 0 ? '+' : ''}${pawns.toFixed(2)}`
}
