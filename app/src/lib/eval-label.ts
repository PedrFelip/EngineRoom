/**
 * Formatação da avaliação para a EvalBar: nota (peões), mate-em-N e resultado
 * final. Mate é recuperado do cp sentinela gerado por `scoreToCp`/`terminalCp`
 * (±(100000−N)), sem mudar o schema — funciona retroativamente com partidas
 * já salvas no store.
 */

import { Chess } from "chess.js";

/** Teto da faixa de mate. `scoreToCp` só produz cp nesta faixa a partir de mate. */
const MATE_FLOOR = 99_000;
/** Base da sentinela: mate N vira ±(100000 − N). */
const MATE_BASE = 100_000;

/** Recupera mate-em-N do cp sentinela, ou null se for uma nota comum. */
export function cpToMate(cp: number): number | null {
  const abs = Math.abs(cp);
  if (abs < MATE_FLOOR) return null;
  const mate = Math.sign(cp) * (MATE_BASE - abs);
  // Normaliza -0 (de Math.sign(-100000) * 0) para +0.
  return mate === 0 ? 0 : mate;
}

/** Peões com sinal: "+1.4"/"-0.8". Zero sem sinal. Acima de 10 vira inteiro. */
export function formatCp(cp: number): string {
  if (cp === 0) return "0.0";
  const pawns = cp / 100;
  const sign = pawns > 0 ? "+" : "-";
  const abs = Math.abs(pawns);
  const text = abs < 10 ? abs.toFixed(1) : String(Math.round(abs));
  return sign + text;
}

/** Mate em N: brancas 'M3', pretas '-M7'. */
export function formatMate(mate: number): string {
  const prefix = mate > 0 ? "M" : "-M";
  return prefix + Math.abs(mate);
}

/** Resultado final de uma posição terminal, ou null se o jogo segue. */
export function finalResultLabel(fen: string): "1-0" | "0-1" | "½-½" | null {
  try {
    const c = new Chess(fen);
    if (c.isCheckmate()) return c.turn() === "w" ? "0-1" : "1-0";
    if (c.isGameOver()) return "½-½";
    return null;
  } catch {
    return null;
  }
}

/**
 * Cor do lado a jogar em positions[ply]. positions[ply] é a posição após `ply`
 * lances, então o lado a jogar é a cor do próximo lance (moves[ply].color); na
 * última posição (ply == len) é o oposto do último lance.
 */
export function sideToMoveAtPly(
  moves: { color: "w" | "b" }[],
  ply: number,
): "w" | "b" {
  if (moves.length === 0) return "w";
  if (ply < moves.length) return moves[ply].color;
  const last = moves[moves.length - 1].color;
  return last === "w" ? "b" : "w";
}

/**
 * Rótulo da EvalBar para uma posição. Prioridade:
 *  1. resultado final (posição terminal)
 *  2. mate em N (cp sentinela), POV das brancas
 *  3. nota em peões, POV das brancas
 *
 * `cp` chega do POV do lado a jogar (raw do engine); `stm` normaliza p/ brancas.
 */
export function evalLabel(cp: number, fen: string, stm: "w" | "b"): string {
  const final = finalResultLabel(fen);
  if (final) return final;

  const cpWhite = stm === "w" ? cp : -cp;
  const mate = cpToMate(cpWhite);
  if (mate !== null) return formatMate(mate);

  return formatCp(cpWhite);
}
