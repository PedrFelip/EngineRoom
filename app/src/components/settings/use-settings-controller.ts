import { useEffect, useRef } from 'react'
import { useStore } from 'zustand'
import { runUi } from '../../lib/effect/ui-runtime'
import { probeEngine } from '../../lib/engine'
import { clearGames } from '../../lib/games'
import {
  createSettingsController,
  type SettingsIO,
} from '../../lib/settings-controller'
import { clearCache, getStorageStats } from '../../lib/storage'

const defaultIO: SettingsIO = {
  probe: (signal) => runUi(probeEngine(), signal),
  stats: (signal) => runUi(getStorageStats(), signal),
  clearCache: () => runUi(clearCache()),
  clearGames: () => runUi(clearGames()),
}

export function useSettingsController(
  isOpen: boolean,
  onGamesCleared: (() => void) | undefined,
  closeConfirmation: () => void,
  io: SettingsIO = defaultIO,
) {
  const callbacks = useRef({ onGamesCleared, closeConfirmation })
  callbacks.current = { onGamesCleared, closeConfirmation }
  const controller = useRef<ReturnType<typeof createSettingsController> | null>(
    null,
  )
  if (!controller.current)
    controller.current = createSettingsController(
      io,
      () => callbacks.current.onGamesCleared?.(),
      () => callbacks.current.closeConfirmation(),
    )
  const instance = controller.current
  const state = useStore(instance.store)
  useEffect(() => {
    if (!isOpen) return
    instance.open()
    return instance.close
  }, [instance, isOpen])
  return {
    ...state,
    test: instance.test,
    runClearCache: () => instance.clear('cache'),
    runClearGames: () => instance.clear('games'),
  }
}
