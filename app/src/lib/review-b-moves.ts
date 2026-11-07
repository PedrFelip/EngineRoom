import type { Move } from 'chess.js'

/** Number each move from its actual starting FEN, including black-to-move setups. */
export function bMoveLabel(
  move: Pick<Move, 'before' | 'color' | 'san'>,
): string {
  const fullmove = move.before.split(' ')[5]
  return `${fullmove}${move.color === 'w' ? '.' : '...'} ${move.san}`
}
