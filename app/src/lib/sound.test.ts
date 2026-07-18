import { describe, expect, it, vi } from 'vitest'
import { classifyMove, playMoveSound } from './sound'

describe('classifyMove', () => {
  it('lance silencioso (sem captura, xeque, roque ou promoção) é move', () => {
    expect(classifyMove('Nf3')).toBe('move')
  })

  it('lance com `x` é capture', () => {
    expect(classifyMove('exd5')).toBe('capture')
  })

  it('roque (O-O e O-O-O) usa o som padrão (move)', () => {
    expect(classifyMove('O-O')).toBe('move')
    expect(classifyMove('O-O-O')).toBe('move')
  })

  it('lance com `+` usa o som padrão (move)', () => {
    expect(classifyMove('Qd1+')).toBe('move')
  })

  it('promoção simples (`=`) usa o som padrão (move)', () => {
    expect(classifyMove('e8=Q')).toBe('move')
  })

  it('captura com xeque prioriza capture (`Qxe7+`)', () => {
    expect(classifyMove('Qxe7+')).toBe('capture')
  })

  it('promoção com captura cai em capture (`exd8=Q`)', () => {
    expect(classifyMove('exd8=Q')).toBe('capture')
  })

  it('xeque-mate sem captura usa o som padrão (move)', () => {
    expect(classifyMove('Qh5#')).toBe('move')
  })

  it('mate com captura cai em capture (`Qxe7#`)', () => {
    expect(classifyMove('Qxe7#')).toBe('capture')
  })
})

describe('playMoveSound', () => {
  it('classifica o SAN e delega ao player com (tipo, volume)', () => {
    const play = vi.fn()
    playMoveSound('exd5', 0.5, play)
    expect(play).toHaveBeenCalledWith('capture', 0.5)
  })
})
