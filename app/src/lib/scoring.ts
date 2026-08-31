/**
 * Conversão de avaliação (centipawns, POV do lado a jogar) em:
 *  - probabilidade de vitória (win%) via curva logística;
 *  - classificação de lances (Brilhante/Melhor/Excelente/Bom/Imprecisão/Erro/
 *    Blunder/Livro);
 *  - precisão agregada da partida (0–100%) pelo modelo completo do Lichess.
 *
 * Classificação e precisão usam delta de win%. A precisão da partida combina
 * accuracy por lance, volatilidade e média harmônica. Mantido puro e sem
 * efeitos colaterais.
 */

import type { AccuracyByColor, Classification } from '../types'

export type { Classification }

/** Inclinação da curva logística cp→win% (constante do modelo Lichess). */
const WINPCT_K = 0.00368208
/** O Lichess limita avaliações antes de convertê-las em chance de vitória. */
export const LICHESS_CP_CEILING = 1000
/** Avaliação convencional da posição inicial usada pelo agregador do Lichess. */
export const LICHESS_INITIAL_CP = 15

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
  const ceiledCp = Math.min(
    LICHESS_CP_CEILING,
    Math.max(-LICHESS_CP_CEILING, cp),
  )
  return 50 + 50 * (2 / (1 + Math.exp(-WINPCT_K * ceiledCp)) - 1)
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

/**
 * Perda em centipawns do lance jogado contra o melhor lance da posição.
 *
 * As duas avaliações chegam no POV do lado a jogar. Depois do lance o turno
 * muda, portanto `-afterCp` é a avaliação do lance jogado no mesmo POV de
 * `bestCp`. Valores negativos são ruído de buscas independentes e viram zero.
 */
export function centipawnLoss(bestCp: number, afterCp: number): number {
  return Math.max(0, bestCp + afterCp)
}

/** Identifica revisões persistidas que já usam o algoritmo atual. */
export const ACCURACY_MODEL_VERSION = 'lichess-2026-08'

const MOVE_ACCURACY_CEIL = 103.1668100711649
const MOVE_ACCURACY_DECAY = 0.04354415386753951
const MOVE_ACCURACY_OFFSET = -3.166924740191411
const UNCERTAINTY_BONUS = 1
const MIN_VOLATILITY_WEIGHT = 0.5
const MAX_VOLATILITY_WEIGHT = 12

/**
 * Accuracy de um lance a partir da perda de Win%. Constantes e bônus de
 * incerteza reproduzem `AccuracyPercent.fromWinPercents` do Lichess.
 */
export function moveAccuracy(winPctLoss: number): number {
  if (winPctLoss <= 0) return 100
  const raw =
    MOVE_ACCURACY_CEIL * Math.exp(-MOVE_ACCURACY_DECAY * winPctLoss) +
    MOVE_ACCURACY_OFFSET +
    UNCERTAINTY_BONUS
  return Math.min(100, Math.max(0, raw))
}

function standardDeviation(values: number[]): number {
  if (values.length === 0) return 0
  const mean = values.reduce((sum, value) => sum + value, 0) / values.length
  const variance =
    values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length
  return Math.sqrt(variance)
}

/**
 * Peso de cada ply pela volatilidade de Win% numa janela deslizante. A janela
 * cresce com a partida (2–8 posições) e o peso é limitado a 0,5–12, exatamente
 * como no agregador do Lichess.
 */
export function lichessVolatilityWeights(
  positionWinPcts: number[],
  moveCount: number,
): number[] {
  if (moveCount === 0) return []
  const values = positionWinPcts.slice(0, moveCount + 1)
  const windowSize = Math.min(8, Math.max(2, Math.floor(moveCount / 10)))
  const windows: number[][] = []
  const repeatedInitialWindows = Math.max(
    0,
    Math.min(windowSize, values.length) - 2,
  )

  for (let i = 0; i < repeatedInitialWindows; i++) {
    windows.push(values.slice(0, windowSize))
  }
  for (let start = 0; start + windowSize <= values.length; start++) {
    windows.push(values.slice(start, start + windowSize))
  }

  const weights = windows.map((window) =>
    Math.min(
      MAX_VOLATILITY_WEIGHT,
      Math.max(MIN_VOLATILITY_WEIGHT, standardDeviation(window)),
    ),
  )
  while (weights.length < moveCount) weights.push(MIN_VOLATILITY_WEIGHT)
  return weights.slice(0, moveCount)
}

interface AccuracyMove {
  color: 'w' | 'b'
}

function aggregateAccuracies(
  accuracies: { value: number; weight: number }[],
): number {
  if (accuracies.length === 0) return 100
  const weightSum = accuracies.reduce((sum, item) => sum + item.weight, 0)
  const weightedMean =
    accuracies.reduce((sum, item) => sum + item.value * item.weight, 0) /
    weightSum
  const harmonicMean =
    accuracies.length /
    accuracies.reduce((sum, item) => sum + 1 / Math.max(1, item.value), 0)
  return (weightedMean + harmonicMean) / 2
}

/**
 * Accuracy completa por cor. Calcula a accuracy de cada lance e tira a média
 * entre (a) média ponderada pela volatilidade e (b) média harmônica.
 */
export function gameAccuracy(
  moves: AccuracyMove[],
  positionWinPcts: number[],
): AccuracyByColor {
  const winPcts = [
    cpToWinPct(LICHESS_INITIAL_CP),
    ...positionWinPcts.slice(1, moves.length + 1),
  ]
  const weights = lichessVolatilityWeights(winPcts, moves.length)
  const forColor = (color: 'w' | 'b') =>
    aggregateAccuracies(
      moves.flatMap((move, index) =>
        move.color === color
          ? [
              {
                value: moveAccuracy(
                  Math.max(
                    0,
                    move.color === 'w'
                      ? winPcts[index] - winPcts[index + 1]
                      : winPcts[index + 1] - winPcts[index],
                  ),
                ),
                weight: weights[index],
              },
            ]
          : [],
      ),
    )
  return { white: forColor('w'), black: forColor('b') }
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
