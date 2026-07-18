import { describe, expect, it } from 'vitest'
import { estimateTimeSeconds } from '../components/EngineTierSelector'

describe('estimateTimeSeconds', () => {
  it('multiplica (plies + 1) posições pelo tempo por lance, em segundos', () => {
    // 40 lances (plies) → 41 posições; 5s/lance = 205s
    expect(estimateTimeSeconds(40, 5000)).toBe(205)
  })

  it('para 0 lances, considera ao menos a posição inicial', () => {
    expect(estimateTimeSeconds(0, 5000)).toBe(5)
  })

  it('reflete mudanças no tempo por lance linearmente', () => {
    expect(estimateTimeSeconds(20, 1000)).toBe(21)
    expect(estimateTimeSeconds(20, 3000)).toBe(63)
    expect(estimateTimeSeconds(20, 30000)).toBe(630)
  })
})
