import { Chess } from 'chess.js'
import { describe, expect, it } from 'vitest'
import { bMoveLabel } from './review-b-moves'

describe('experimental B move labels', () => {
  it('uses the move number and side from a setup position', () => {
    const game = new Chess()
    game.loadPgn(
      '[SetUp "1"]\n[FEN "8/8/8/8/8/4k3/8/4K3 b - - 0 42"]\n\n42... Kd3 *',
    )
    expect(bMoveLabel(game.history({ verbose: true })[0])).toBe('42... Kd3')
  })

  it('numbers both sides correctly in an ordinary game', () => {
    const game = new Chess()
    game.loadPgn('1. e4 e5 2. Nf3 *')
    expect(game.history({ verbose: true }).map(bMoveLabel)).toEqual([
      '1. e4',
      '1... e5',
      '2. Nf3',
    ])
  })
})
