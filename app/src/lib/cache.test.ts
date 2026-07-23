import { describe, expect, it } from 'vitest'
import { shapeCachedPosition } from './cache'

const FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

describe('shapeCachedPosition', () => {
  it('fatia as linhas ao multipv pedido e reconstrói depth real da pv-1', () => {
    const dto = {
      cp: 35,
      linesJson: JSON.stringify([
        { multipv: 1, cp: 35, pv: ['e2e4'], san: 'e4', depth: 28 },
        { multipv: 2, cp: 30, pv: ['d2d4'], san: 'd4', depth: 27 },
        { multipv: 3, cp: 28, pv: ['c2c4'], san: 'c4', depth: 27 },
      ]),
      reachedDepth: 28,
    }

    const pos = shapeCachedPosition(dto, FEN, 2)

    expect(pos.lines).toHaveLength(2)
    expect(pos.depth).toBe(28)
    expect(pos.pv).toEqual(['e2e4'])
    expect(pos.cp).toBe(35)
  })

  it('usa reachedDepth real, não o escalar do pedido (fix bug cache.ts:26)', () => {
    // Hit de modo tempo: reachedDepth=28 (plies), mas o pedido foi 5000 (ms).
    // O depth reconstruído deve ser 28, nunca 5000.
    const dto = {
      cp: 35,
      linesJson: JSON.stringify([
        { multipv: 1, cp: 35, pv: ['e2e4'], depth: 28 },
      ]),
      reachedDepth: 28,
    }

    const pos = shapeCachedPosition(dto, FEN, 1)

    expect(pos.depth).toBe(28)
  })

  it('cai para reachedDepth quando as linhas não trazem depth (legado)', () => {
    const dto = {
      cp: 35,
      linesJson: JSON.stringify([{ multipv: 1, cp: 35, pv: ['e2e4'] }]),
      reachedDepth: 20,
    }

    const pos = shapeCachedPosition(dto, FEN, 1)

    expect(pos.depth).toBe(20)
  })

  it('preserva o pv principal mesmo quando multipv pedido é 1', () => {
    const dto = {
      cp: 35,
      linesJson: JSON.stringify([
        { multipv: 1, cp: 35, pv: ['e2e4'], depth: 28 },
        { multipv: 2, cp: 30, pv: ['d2d4'], depth: 27 },
      ]),
      reachedDepth: 28,
    }

    const pos = shapeCachedPosition(dto, FEN, 1)

    expect(pos.lines).toHaveLength(1)
    expect(pos.pv).toEqual(['e2e4'])
  })
})
