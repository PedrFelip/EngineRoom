/** Funções puras de avaliação, classificação e precisão baseadas no Lichess. */

import type { AccuracyByColor, Classification } from '../types'

export type { Classification }

/** Inclinação da curva logística cp→win% (constante do modelo Lichess). */
const WINPCT_K = 0.00368208
/** O Lichess limita avaliações antes de convertê-las em chance de vitória. */
export const LICHESS_CP_CEILING = 1000
/** Avaliação convencional da posição inicial usada pelo agregador do Lichess. */
export const LICHESS_INITIAL_CP = 15

export const CLASSIFICATION_LABELS: Record<Classification, string> = {
  livro: 'Livro',
  melhor: 'Melhor',
  excelente: 'Excelente',
  bom: 'Bom',
  imprecisao: 'Imprecisão',
  erro: 'Erro',
  blunder: 'Blunder',
}

/** Converte centipawns do lado a jogar em win% pela curva do Lichess. */
export function cpToWinPct(cp: number): number {
  const ceiledCp = Math.min(
    LICHESS_CP_CEILING,
    Math.max(-LICHESS_CP_CEILING, cp),
  )
  return 50 + 50 * (2 / (1 + Math.exp(-WINPCT_K * ceiledCp)) - 1)
}

/** Normaliza centipawns do lado a jogar para o POV das brancas. */
export function whiteCp(cp: number, stm: 'w' | 'b'): number {
  return stm === 'w' ? cp : -cp
}

/** Converte centipawns do lado a jogar em win% do POV das brancas. */
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

/** Limites de perda de win% para as classificações comuns. */
const EXCELLENT_MAX_LOSS = 2
const GOOD_MAX_LOSS = 5
const INACCURACY_MAX_LOSS = 10
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
