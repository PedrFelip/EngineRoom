import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from 'react'
import { useStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { loadSettings, saveSettings, type Theme } from './settings'
import { createSettingsStore, type SettingsState } from './settings-store'

const SettingsContext = createContext<ReturnType<
  typeof createSettingsStore
> | null>(null)

function applyTheme(theme: Theme) {
  const root = document.documentElement
  root.classList.toggle('light', theme === 'light')
  root.classList.toggle('dark', theme === 'dark')
  root.style.colorScheme = theme
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [store] = useState(() => createSettingsStore(loadSettings()))

  useEffect(() => {
    applyTheme(store.getState().settings.theme)
    saveSettings(store.getState().settings)
    return store.subscribe((state, previous) => {
      if (state.settings.theme !== previous.settings.theme) {
        applyTheme(state.settings.theme)
      }
      saveSettings(state.settings)
    })
  }, [store])

  return (
    <SettingsContext.Provider value={store}>
      {children}
    </SettingsContext.Provider>
  )
}

export function useSettings<T>(selector: (state: SettingsState) => T): T {
  const store = useContext(SettingsContext)
  if (!store)
    throw new Error('useSettings precisa estar dentro de <SettingsProvider>')
  return useStore(store, useShallow(selector))
}
