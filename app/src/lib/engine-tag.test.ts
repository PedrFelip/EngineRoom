import { describe, expect, it } from 'vitest'
import { formatEngineTag } from './engine-tag'

describe('formatEngineTag', () => {
  it('mostra tier + depth no modo profundidade', () => {
    expect(
      formatEngineTag({ mode: 'depth', depth: 20, engineTier: 'balanced' }),
    ).toBe('Equilibrado · d20')
  })

  it('cai para d{depth} quando o tier é desconhecido', () => {
    expect(
      formatEngineTag({ mode: 'depth', depth: 18, engineTier: 'legado' }),
    ).toBe('d18 · d18')
  })

  it('mostra segundos por lance no modo tempo', () => {
    expect(
      formatEngineTag({ mode: 'time', depth: 5000, engineTier: 'time' }),
    ).toBe('Tempo · 5s/lance')
  })

  it('converte milissegundos ímpares para segundos arredondados', () => {
    expect(
      formatEngineTag({ mode: 'time', depth: 12000, engineTier: 'time' }),
    ).toBe('Tempo · 12s/lance')
  })
})
