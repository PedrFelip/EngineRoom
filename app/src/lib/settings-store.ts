import { createStore } from 'zustand/vanilla'
import { DEFAULT_SETTINGS, type Settings, type Theme } from './settings'

export interface SettingsState {
  settings: Settings
  updateSettings: (patch: Partial<Settings>) => void
  setTheme: (theme: Theme) => void
  setSoundEnabled: (enabled: boolean) => void
  setSoundVolume: (volume: number) => void
  reset: () => void
}

export function createSettingsStore(initial: Settings = DEFAULT_SETTINGS) {
  return createStore<SettingsState>()((set) => {
    function updateSettings(patch: Partial<Settings>): void {
      set((state) => {
        const changed = Object.entries(patch).some(
          ([key, value]) => state.settings[key as keyof Settings] !== value,
        )
        return changed ? { settings: { ...state.settings, ...patch } } : state
      })
    }

    return {
      settings: { ...initial },
      updateSettings,
      setTheme: (theme) => updateSettings({ theme }),
      setSoundEnabled: (soundEnabled) => updateSettings({ soundEnabled }),
      setSoundVolume: (volume) =>
        updateSettings({ soundVolume: Math.max(0, Math.min(1, volume)) }),
      reset: () => updateSettings(DEFAULT_SETTINGS),
    }
  })
}

export function selectReviewEngineSettings({ settings }: SettingsState) {
  return {
    reviewEngineEnabled: settings.reviewEngineEnabled,
    reviewMoveFeedbackEnabled: settings.reviewMoveFeedbackEnabled,
    reviewSearchSeconds: settings.reviewSearchSeconds,
    reviewAnalysisLines: settings.reviewAnalysisLines,
    reviewThreadsAuto: settings.reviewThreadsAuto,
    reviewThreads: settings.reviewThreads,
    reviewMemoryMb: settings.reviewMemoryMb,
  }
}
