import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  DEFAULT_SETTINGS,
  loadSettings,
  recommendedReviewThreads,
  reviewThreadSettings,
  saveSettings,
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

describe('usuário salvo do Chess.com', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('restaura e normaliza o último usuário sem perder outras preferências', () => {
    stubStorage(
      JSON.stringify({ chessComUsername: ' Alice_123 ', theme: 'light' }),
    )
    const settings = loadSettings()
    expect(settings.chessComUsername).toBe('alice_123')
    expect(settings.theme).toBe('light')
    saveSettings(settings)
    expect(localStorage.setItem).toHaveBeenCalledWith(
      'engineroom.settings.v1',
      JSON.stringify(settings),
    )
  })

  it('ignora usernames ausentes ou inválidos sem resetar o tema', () => {
    for (const username of [
      undefined,
      null,
      123,
      '',
      '../alice',
      'a'.repeat(51),
    ]) {
      stubStorage(
        JSON.stringify({ chessComUsername: username, theme: 'light' }),
      )
      const settings = loadSettings()
      expect(settings.chessComUsername).toBeNull()
      expect(settings.theme).toBe('light')
    }
  })
})

describe('configurações da análise na revisão', () => {
  it('migra configurações antigas usando os novos padrões', () => {
    stubStorage(JSON.stringify({ theme: 'light' }))
    const settings = loadSettings()
    expect(settings.reviewEngineEnabled).toBe(true)
    expect(settings.reviewMoveFeedbackEnabled).toBe(true)
    expect(settings.reviewSearchSeconds).toBe(8)
    expect(settings.reviewAnalysisLines).toBe(3)
    expect(settings.reviewMemoryMb).toBe(16)
  })

  it('preserva feedback de movimentos desligado', () => {
    stubStorage(JSON.stringify({ reviewMoveFeedbackEnabled: false }))
    expect(loadSettings().reviewMoveFeedbackEnabled).toBe(false)
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
    expect(reviewThreadSettings(true, 32)).toEqual({
      reviewThreadsAuto: true,
      reviewThreads: 11,
    })
    expect(reviewThreadSettings(false, 32)).toEqual({
      reviewThreadsAuto: false,
    })
  })
})
