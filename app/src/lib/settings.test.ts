import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  DEFAULT_SETTINGS,
  loadSettings,
  SETTINGS_KEY,
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
})
