import { useEffect, useRef, useState } from 'react'
import { useStore } from 'zustand'
import { TauriChessComSearch } from './chess-com-import'
import {
  createChessComImportController,
  createChessComImportStore,
} from './chess-com-import-controller'
import { useSettings } from './settings-context'

/** A fresh owner per effect setup also supports React StrictMode's replay. */
export function useChessComImport(
  onImport: (pgn: string) => void,
  active = true,
) {
  const savedUsername = useSettings((state) => state.settings.chessComUsername)
  const updateSettings = useSettings((state) => state.updateSettings)
  const [initialUsername] = useState(savedUsername)
  const [store] = useState(createChessComImportStore)
  const onImportRef = useRef(onImport)
  const closing = useRef(Promise.resolve())
  const owner = useRef<ReturnType<
    typeof createChessComImportController
  > | null>(null)
  const state = useStore(store)
  useEffect(() => {
    onImportRef.current = onImport
  }, [onImport])
  useEffect(() => {
    const controller = createChessComImportController(
      TauriChessComSearch,
      store,
      {
        savedUsername: initialUsername,
        onUsernameSaved: (chessComUsername) =>
          updateSettings({ chessComUsername }),
        onImport: (pgn) => onImportRef.current(pgn),
      },
    )
    owner.current = controller
    return () => {
      owner.current = null
      closing.current = controller.dispose()
    }
  }, [store, initialUsername, updateSettings])
  useEffect(() => {
    let cancelled = false
    if (active) {
      // Replayed mounts wait for the previous owner's acknowledged IPC cleanup.
      void closing.current.then(() => {
        if (!cancelled) return owner.current?.start()
      })
    } else {
      // Deactivation is implicit: a later reactivation may resume this search.
      owner.current?.suspend()
    }
    return () => {
      cancelled = true
    }
  }, [active])
  return {
    state,
    savedUsername,
    setUsername: (username: string) => owner.current?.setUsername(username),
    search: (append = false) => {
      void owner.current?.search(append)
    },
    cancel: () => owner.current?.cancel(),
    select: (url: string) => owner.current?.select(url),
    forgetUsername: () => owner.current?.forgetUsername(),
  }
}
