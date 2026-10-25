import { Effect, type Layer } from 'effect'
import { createStore } from 'zustand/vanilla'
import { normalizeChessComUsername } from './chess-com-account'
import {
  ChessComImportError,
  ChessComSearch,
  type RemoteGame,
  type SearchCursor,
  type SearchResult,
} from './chess-com-import'
import { errorMessage } from './effect/errors'
import { runUi } from './effect/ui-runtime'

interface ImportState {
  username: string
  searchedUsername: string | null
  loading: boolean
  items: readonly RemoteGame[]
  selectedUrl: string | null
  cursor: SearchCursor | null
  result: SearchResult | null
  error: string | null
  coolingDown: boolean
  retryAfterSeconds: number | null
}
const initial: ImportState = {
  username: '',
  searchedUsername: null,
  loading: false,
  items: [],
  selectedUrl: null,
  cursor: null,
  result: null,
  error: null,
  coolingDown: false,
  retryAfterSeconds: null,
}
export function createChessComImportStore() {
  return createStore<ImportState>(() => initial)
}

/** UI intents are synchronous; all IPC, cancellation and timers run here. */
export function createChessComImportController(
  backend: Layer.Layer<ChessComSearch>,
  store = createChessComImportStore(),
  options: {
    savedUsername?: string | null
    onUsernameSaved?: (username: string | null) => void
    onImport?: (pgn: string) => void
  } = {},
) {
  let current: AbortController | undefined
  let cooldown: AbortController | undefined
  let cooldownTask = Promise.resolve()
  let tail = Promise.resolve()
  let disposed = false
  let initialized = false
  let interacted = false

  function cancel() {
    current?.abort()
    current = undefined
    store.setState({ loading: false })
  }
  function coolDown(seconds: number | null) {
    const wait = Math.max(1, Math.min(seconds ?? 60, 86400))
    cooldown?.abort()
    const owner = new AbortController()
    cooldown = owner
    store.setState({ coolingDown: true, retryAfterSeconds: wait })
    cooldownTask = runUi(Effect.sleep(wait * 1000), owner.signal)
      .then(() => {
        if (!disposed && cooldown === owner) {
          store.setState({ coolingDown: false, retryAfterSeconds: null })
        }
      })
      .catch(() => {})
  }

  const controller = {
    store,
    start(): Promise<void> {
      if (disposed || initialized) return Promise.resolve()
      initialized = true
      const saved = normalizeChessComUsername(options.savedUsername)
      if (!saved || interacted) return Promise.resolve()
      store.setState({ username: saved })
      return controller.search()
    },
    setUsername(username: string) {
      if (disposed) return
      interacted = true
      cancel()
      store.setState({
        ...initial,
        username,
        coolingDown: store.getState().coolingDown,
        retryAfterSeconds: store.getState().retryAfterSeconds,
      })
    },
    cancel,
    select(url: string) {
      if (disposed || store.getState().selectedUrl === url) return
      const game = store.getState().items.find((item) => item.url === url)
      if (!game) return
      store.setState({ selectedUrl: url })
      options.onImport?.(game.pgn)
    },
    forgetUsername() {
      if (disposed) return
      controller.setUsername('')
      options.onUsernameSaved?.(null)
    },
    selectedPgn() {
      const state = store.getState()
      return (
        state.items.find((game) => game.url === state.selectedUrl)?.pgn ?? null
      )
    },
    search(append = false): Promise<void> {
      if (disposed || store.getState().coolingDown) return Promise.resolve()
      interacted = true
      const state = store.getState()
      const username = normalizeChessComUsername(state.username)
      if (!username) {
        store.setState({
          error: 'Informe um nome de usuário válido do Chess.com.',
        })
        return Promise.resolve()
      }
      if (append && (!state.cursor || state.items.length >= 100))
        return Promise.resolve()
      cancel()
      const owner = new AbortController()
      current = owner
      const cursor = append ? state.cursor : null
      store.setState({
        loading: true,
        error: null,
        ...(append
          ? {}
          : { items: [], cursor: null, result: null, selectedUrl: null }),
      })
      // Acquisition waits for the previous request's acknowledged cleanup.
      tail = tail.then(async () => {
        if (owner.signal.aborted || disposed) return
        try {
          const result = await runUi(
            Effect.gen(function* () {
              const service = yield* ChessComSearch
              return yield* service.search(username, cursor)
            }).pipe(Effect.provide(backend)),
            owner.signal,
          )
          if (owner.signal.aborted || disposed || current !== owner) return
          const items = append ? [...store.getState().items] : []
          const seen = new Set(items.map((game) => game.url))
          for (const game of result.items) {
            if (!seen.has(game.url) && items.length < 100) {
              items.push(game)
              seen.add(game.url)
            }
          }
          store.setState({
            items,
            result,
            cursor: items.length < 100 ? result.nextCursor : null,
            searchedUsername: username,
          })
          if (!append) options.onUsernameSaved?.(username)
          if (result.warning?.code === 'rateLimited') {
            coolDown(result.warning.retryAfterSeconds)
          }
        } catch (error) {
          if (owner.signal.aborted || disposed || current !== owner) return
          store.setState({ error: errorMessage(error) })
          if (
            error instanceof ChessComImportError &&
            error.code === 'rateLimited'
          ) {
            coolDown(error.retryAfterSeconds)
          }
        } finally {
          if (!disposed && current === owner) {
            current = undefined
            store.setState({ loading: false })
          }
        }
      })
      return tail
    },
    async dispose() {
      cancel()
      disposed = true
      cooldown?.abort()
      store.setState({ coolingDown: false, retryAfterSeconds: null })
      await Promise.all([tail, cooldownTask])
    },
  }
  return controller
}
