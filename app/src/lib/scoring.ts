/**
 * Conversão de avaliação (centipawns, POV do lado a jogar) em:
 *  - probabilidade de vitória (win%) via curva logística;
 *  - classificação de lances (Melhor/Excelente/Bom/Imprecisão/Erro/Blunder/Livro);
 *  - precisão agregada da partida (0–100%).
 *
 * Estilo chess.com: thresholds sobre delta de win% (adaptativos à complexidade)
 * e fórmula de precisão por lance. Mantido puro e sem efeitos colaterais.
 */

import type { Classification } from "../types";

export type { Classification };

/** Inclinação da curva logística cp→win% (constante do modelo chess.com). */
const WINPCT_K = 0.00368208;

/** Rótulos em pt-BR exibidos na UI (badges, resumo). */
export const CLASSIFICATION_LABELS: Record<Classification, string> = {
  livro: "Livro",
  melhor: "Melhor",
  excelente: "Excelente",
  bom: "Bom",
  imprecisao: "Imprecisão",
  erro: "Erro",
  blunder: "Blunder",
};

/**
 * Converte centipawns (POV do lado a jogar) em probabilidade de vitória (0–100%).
 * Curva logística centrada em 50% para cp = 0.
 */
export function cpToWinPct(cp: number): number {
  return 50 + 50 * (2 / (1 + Math.exp(-WINPCT_K * cp)) - 1);
}

/** Limiar (em delta de win%) abaixo do qual o lance é Excelente. */
const EXCELLENT_MAX_LOSS = 2;
/** Limiar (em delta de win%) abaixo do qual o lance é Bom. */
const GOOD_MAX_LOSS = 5;
/** Limiar (em delta de win%) abaixo do qual o lance é Imprecisão. */
const INACCURACY_MAX_LOSS = 10;
/** Limiar (em delta de win%) abaixo do qual o lance é Erro. */
const MISTAKE_MAX_LOSS = 20;

/**
 * Classifica um lance pela perda de win% em relação ao melhor lance (delta de win%).
 * Acima de MISTAKE_MAX_LOSS o lance é Blunder. Lance de abertura (isBook) é Livro.
 */
export function classifyMove(
  winPctLoss: number,
  isBook = false,
): Classification {
  if (isBook) return "livro";
  if (winPctLoss <= 0) return "melhor";
  if (winPctLoss <= EXCELLENT_MAX_LOSS) return "excelente";
  if (winPctLoss <= GOOD_MAX_LOSS) return "bom";
  if (winPctLoss <= INACCURACY_MAX_LOSS) return "imprecisao";
  if (winPctLoss <= MISTAKE_MAX_LOSS) return "erro";
  return "blunder";
}

/** Constantes da fórmula de acurácia por lance (modelo chess.com). */
const ACCURACY_CEIL = 103.1668;
const ACCURACY_DECAY = 0.04354;

function moveAccuracy(winPctLoss: number): number {
  return Math.min(100, ACCURACY_CEIL * Math.exp(-ACCURACY_DECAY * (winPctLoss - 1)));
}

/**
 * Precisão agregada da partida (0–100%) a partir das perdas de win% por lance.
 */
export function gameAccuracy(winPctLosses: number[]): number {
  if (winPctLosses.length === 0) return 100;
  const sum = winPctLosses.reduce((acc, loss) => acc + moveAccuracy(loss), 0);
  return sum / winPctLosses.length;
}
