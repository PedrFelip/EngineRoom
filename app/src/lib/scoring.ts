import type { Classification } from '../types'

export type { Classification }

export const CLASSIFICATION_LABELS: Record<Classification, string> = {
  livro: 'Livro',
  melhor: 'Melhor',
  excelente: 'Excelente',
  bom: 'Bom',
  imprecisao: 'Imprecisão',
  erro: 'Erro',
  blunder: 'Blunder',
}

export function whiteCp(cp: number, stm: 'w' | 'b'): number {
  return stm === 'w' ? cp : -cp
}

export function sideToMoveAtPly(
  moves: { color: 'w' | 'b' }[],
  ply: number,
): 'w' | 'b' {
  if (moves.length === 0) return 'w'
  if (ply < moves.length) return moves[ply].color
  const last = moves[moves.length - 1].color
  return last === 'w' ? 'b' : 'w'
}

const MATE_CP = 90000

export function formatEval(cp: number): string {
  if (cp >= MATE_CP) return `#${100000 - cp}`
  if (cp <= -MATE_CP) return `-#${100000 + cp}`
  const pawns = cp / 100
  return `${pawns >= 0 ? '+' : ''}${pawns.toFixed(2)}`
}
