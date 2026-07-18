import { describe, expect, it } from 'vitest'
import { resolveEngineTier } from './engine-tier'

describe('resolveEngineTier', () => {
  it('devolve o tier preset cuja depth bate exatamente', () => {
    expect(resolveEngineTier(15).id).toBe('fast')
    expect(resolveEngineTier(20).id).toBe('balanced')
    expect(resolveEngineTier(25).id).toBe('deep')
  })

  it('para depth fora dos presets, devolve tier custom com a depth informada', () => {
    const t = resolveEngineTier(17)
    expect(t.id).toBe('custom')
    expect(t.depth).toBe(17)
    expect(t.label).toBeTruthy()
  })

  it('clamp: depth abaixo de 15 vira 15, acima de 25 vira 25', () => {
    expect(resolveEngineTier(10).depth).toBe(15)
    expect(resolveEngineTier(10).id).toBe('fast')
    expect(resolveEngineTier(30).depth).toBe(25)
    expect(resolveEngineTier(30).id).toBe('deep')
  })
})
