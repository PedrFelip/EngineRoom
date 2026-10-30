import { createStore } from 'zustand/vanilla'
import type { ProbeResult } from './engine'
import type { StorageStats } from './storage'

export interface SettingsIO {
  probe: (signal: AbortSignal) => Promise<ProbeResult>
  stats: (signal: AbortSignal) => Promise<StorageStats>
  clearCache: () => Promise<void>
  clearGames: () => Promise<void>
}

interface State {
  testing: boolean
  result: ProbeResult | null
  stats: StorageStats | null
  acting: boolean
  storageError: string | null
}

export function createSettingsController(
  io: SettingsIO,
  onGamesCleared: () => void,
  onClearFinished: () => void,
) {
  const store = createStore<State>(() => ({
    testing: false,
    result: null,
    stats: null,
    acting: false,
    storageError: null,
  }))
  let generation = 0
  let active = false
  let probe: AbortController | undefined
  let statsRequest: AbortController | undefined
  let clearing = false
  const current = (epoch: number) => active && generation === epoch

  function refresh(epoch: number, reportError: boolean) {
    statsRequest?.abort()
    const request = new AbortController()
    statsRequest = request
    return io
      .stats(request.signal)
      .then((stats) => {
        if (current(epoch) && !request.signal.aborted) store.setState({ stats })
      })
      .catch((error) => {
        if (!current(epoch) || request.signal.aborted) return
        console.warn('Falha ao ler armazenamento:', error)
        if (reportError)
          store.setState({ storageError: 'Não foi possível ler o tamanho.' })
      })
  }
  function open() {
    active = true
    const epoch = ++generation
    store.setState({
      testing: false,
      result: null,
      stats: null,
      storageError: null,
      acting: clearing,
    })
    void refresh(epoch, true)
  }
  function close() {
    active = false
    generation++
    probe?.abort()
    probe = undefined
    statsRequest?.abort()
    statsRequest = undefined
  }
  function test() {
    if (!active) return
    probe?.abort()
    const request = new AbortController()
    probe = request
    const epoch = generation
    store.setState({ testing: true, result: null })
    void io
      .probe(request.signal)
      .then((result) => {
        if (current(epoch) && !request.signal.aborted)
          store.setState({ result })
      })
      .catch((error) => {
        if (current(epoch) && !request.signal.aborted)
          store.setState({
            result: { ok: false, name: null, error: String(error) },
          })
      })
      .finally(() => {
        if (probe === request) {
          probe = undefined
          if (current(epoch)) store.setState({ testing: false })
        }
      })
  }
  async function clear(target: 'cache' | 'games') {
    if (!active || clearing) return
    clearing = true
    const epoch = generation
    statsRequest?.abort()
    store.setState({ acting: true })
    try {
      await (target === 'cache' ? io.clearCache() : io.clearGames())
      if (target === 'games') onGamesCleared()
      if (current(epoch)) {
        store.setState({ storageError: null })
        await refresh(epoch, false)
      }
    } catch (error) {
      if (current(epoch)) {
        console.warn('Falha ao limpar armazenamento:', error)
        store.setState({
          storageError:
            target === 'cache'
              ? 'Falha ao limpar o cache.'
              : 'Falha ao limpar o histórico.',
        })
      }
    } finally {
      clearing = false
      if (active) {
        store.setState({ acting: false })
        if (current(epoch)) onClearFinished()
        else void refresh(generation, false)
      }
    }
  }
  return { store, open, close, test, clear }
}
