import { describe, expect, it, vi } from 'vitest'
import { shallow } from 'zustand/shallow'
import { DEFAULT_SETTINGS } from './settings'
import {
  createSettingsStore,
  selectReviewEngineSettings,
} from './settings-store'

describe('settings store', () => {
  it('preserva preferências e limita volume, com ações estáveis', () => {
    const store = createSettingsStore({ ...DEFAULT_SETTINGS, theme: 'light' })
    const actions = store.getState()
    actions.setSoundVolume(2)
    expect(store.getState().settings.soundVolume).toBe(1)
    expect(store.getState().settings.theme).toBe('light')
    expect(store.getState().updateSettings).toBe(actions.updateSettings)
    actions.reset()
    expect(store.getState().settings).toEqual(DEFAULT_SETTINGS)
  })

  it('não notifica alterações sem efeito nem afeta outra instância', () => {
    const store = createSettingsStore()
    const other = createSettingsStore()
    const listener = vi.fn()
    store.subscribe(listener)
    store.getState().setTheme(DEFAULT_SETTINGS.theme)
    expect(listener).not.toHaveBeenCalled()
    store.getState().setTheme('light')
    expect(listener).toHaveBeenCalledTimes(1)
    expect(other.getState().settings.theme).toBe(DEFAULT_SETTINGS.theme)
  })

  it('isola os seletores do motor das preferências de aparência e som', () => {
    const store = createSettingsStore()
    const before = selectReviewEngineSettings(store.getState())
    store.getState().setTheme('light')
    store.getState().setSoundVolume(0.2)
    expect(shallow(before, selectReviewEngineSettings(store.getState()))).toBe(
      true,
    )
    store.getState().updateSettings({ reviewAnalysisLines: 5 })
    expect(shallow(before, selectReviewEngineSettings(store.getState()))).toBe(
      false,
    )
  })
})
