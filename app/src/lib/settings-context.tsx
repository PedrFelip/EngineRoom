import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react'
import {
  DEFAULT_SETTINGS,
  loadSettings,
  type Settings,
  saveSettings,
  type Theme,
} from './settings'

interface SettingsContextValue {
  settings: Settings
  setTheme: (theme: Theme) => void
  setEnginePath: (path: string) => void
  setSoundEnabled: (enabled: boolean) => void
  setSoundVolume: (volume: number) => void
  reset: () => void
}

const SettingsContext = createContext<SettingsContextValue | null>(null)

function applyTheme(theme: Theme) {
  const root = document.documentElement
  root.classList.toggle('light', theme === 'light')
  root.classList.toggle('dark', theme === 'dark')
  root.style.colorScheme = theme
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(() => loadSettings())

  useEffect(() => {
    applyTheme(settings.theme)
    saveSettings(settings)
  }, [settings])

  const setTheme = useCallback(
    (theme: Theme) => setSettings((s) => ({ ...s, theme })),
    [],
  )
  const setEnginePath = useCallback(
    (path: string) => setSettings((s) => ({ ...s, enginePath: path })),
    [],
  )
  const setSoundEnabled = useCallback(
    (enabled: boolean) => setSettings((s) => ({ ...s, soundEnabled: enabled })),
    [],
  )
  const setSoundVolume = useCallback(
    (volume: number) =>
      setSettings((s) => ({
        ...s,
        soundVolume: Math.max(0, Math.min(1, volume)),
      })),
    [],
  )
  const reset = useCallback(() => setSettings({ ...DEFAULT_SETTINGS }), [])

  const value = useMemo<SettingsContextValue>(
    () => ({
      settings,
      setTheme,
      setEnginePath,
      setSoundEnabled,
      setSoundVolume,
      reset,
    }),
    [settings, setTheme, setEnginePath, setSoundEnabled, setSoundVolume, reset],
  )

  return (
    <SettingsContext.Provider value={value}>
      {children}
    </SettingsContext.Provider>
  )
}

export function useSettings(): SettingsContextValue {
  const ctx = useContext(SettingsContext)
  if (!ctx)
    throw new Error('useSettings precisa estar dentro de <SettingsProvider>')
  return ctx
}
