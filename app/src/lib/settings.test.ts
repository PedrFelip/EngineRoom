import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  DEFAULT_SETTINGS,
  loadSettings,
  recommendedReviewThreads,
} from './settings'

function stubStorage(raw: string | null) {
  vi.stubGlobal('localStorage', {
    getItem: vi.fn(() => raw),
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
  })
}

describe('loadSettings (som)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('sem nada no localStorage, som vem ligado com volume padrão', () => {
    stubStorage(null)
    const s = loadSettings()
    expect(s.soundEnabled).toBe(true)
    expect(s.soundVolume).toBe(DEFAULT_SETTINGS.soundVolume)
  })

  it('lê soundEnabled e soundVolume do localStorage', () => {
    stubStorage(JSON.stringify({ soundEnabled: false, soundVolume: 0.25 }))
    const s = loadSettings()
    expect(s.soundEnabled).toBe(false)
    expect(s.soundVolume).toBe(0.25)
  })

  it('volume acima de 1 é clampado para 1', () => {
    stubStorage(JSON.stringify({ soundVolume: 2 }))
    expect(loadSettings().soundVolume).toBe(1)
  })

  it('volume abaixo de 0 é clampado para 0', () => {
    stubStorage(JSON.stringify({ soundVolume: -0.5 }))
    expect(loadSettings().soundVolume).toBe(0)
  })

  it('volume ausente ou inválido cai no padrão', () => {
    stubStorage(JSON.stringify({ soundVolume: 'alto' }))
    expect(loadSettings().soundVolume).toBe(DEFAULT_SETTINGS.soundVolume)
  })

  it('soundEnabled ausente preserva o default (true)', () => {
    stubStorage(JSON.stringify({ theme: 'dark' }))
    expect(loadSettings().soundEnabled).toBe(true)
  })

  it('ignora enginePath de configurações antigas', () => {
    stubStorage(JSON.stringify({ enginePath: '/usr/bin/stockfish' }))
    const settings = loadSettings()
    expect(settings).toEqual(DEFAULT_SETTINGS)
    expect(settings).not.toHaveProperty('enginePath')
  })
})

describe('configurações da análise na revisão', () => {
  it('migra configurações antigas usando os novos padrões', () => {
    stubStorage(JSON.stringify({ theme: 'light' }))
    const settings = loadSettings()
    expect(settings.reviewEngineEnabled).toBe(true)
    expect(settings.reviewSearchSeconds).toBe(8)
    expect(settings.reviewAnalysisLines).toBe(1)
    expect(settings.reviewMemoryMb).toBe(16)
  })

  it('normaliza valores persistidos fora dos limites', () => {
    stubStorage(
      JSON.stringify({
        reviewSearchSeconds: 99,
        reviewAnalysisLines: 0,
        reviewThreads: 4.6,
        reviewMemoryMb: 8,
      }),
    )
    const settings = loadSettings()
    expect(settings.reviewSearchSeconds).toBe(30)
    expect(settings.reviewAnalysisLines).toBe(1)
    expect(settings.reviewThreads).toBe(5)
    expect(settings.reviewMemoryMb).toBe(16)
  })

  it('recomenda cerca de um terço dos processadores lógicos', () => {
    expect(recommendedReviewThreads(32)).toBe(11)
    expect(recommendedReviewThreads(2)).toBe(1)
  })
})
