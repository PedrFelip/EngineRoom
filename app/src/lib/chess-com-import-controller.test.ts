import { Effect, Layer } from 'effect'
import { describe, expect, it, vi } from 'vitest'
import {
  ChessComImportError,
  ChessComSearch,
  type SearchResult,
} from './chess-com-import'
import {
  createChessComImportController,
  createChessComImportStore,
} from './chess-com-import-controller'
import { parsePgn } from './pgn'

const game = {
  url: 'https://www.chess.com/game/live/123',
  endTime: 1700000000,
  timeClass: 'blitz',
  white: { username: 'API Alice', rating: 1400 },
  black: { username: 'API Bob', rating: 1600 },
  result: '1-0' as const,
  pgn: '[White "PGN Alice"]\n[Black "PGN Bob"]\n[WhiteElo "1300"]\n\n1. e4 e5 1-0',
}
const page: SearchResult = {
  items: [game],
  nextCursor: {
    username: 'alice',
    year: 2026,
    month: 9,
    before: { endTime: game.endTime, url: game.url },
  },
  archivesRead: 1,
  skippedInvalid: 0,
  skippedUnsupported: 0,
  stopReason: 'page',
  warning: null,
}

describe('Chess.com import intents', () => {
  it('loads the saved username automatically and imports immediately on selection', async () => {
    const search = vi.fn(() => Effect.succeed(page))
    const onImport = vi.fn()
    const onUsernameSaved = vi.fn()
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
      createChessComImportStore(),
      { savedUsername: 'Alice', onImport, onUsernameSaved },
    )
    await controller.start()
    await controller.start()
    expect(search).toHaveBeenCalledExactlyOnceWith('alice', null)
    expect(controller.store.getState().username).toBe('alice')
    expect(onUsernameSaved).toHaveBeenCalledExactlyOnceWith('alice')
    expect(onImport).not.toHaveBeenCalled()
    controller.select('unknown')
    controller.select(game.url)
    controller.select(game.url)
    expect(onImport).toHaveBeenCalledExactlyOnceWith(game.pgn)
    await controller.forgetUsername()
    expect(onUsernameSaved).toHaveBeenLastCalledWith(null)
    expect(controller.store.getState().username).toBe('')
    expect(controller.store.getState().items).toEqual([])
    await controller.dispose()
  })

  it('never saves a failed lookup or replaces a username typed before restoration', async () => {
    const search = vi.fn(() =>
      Effect.fail(
        new ChessComImportError({
          code: 'unavailable',
          message: 'Usuário indisponível.',
          retryAfterSeconds: null,
        }),
      ),
    )
    const onUsernameSaved = vi.fn()
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
      createChessComImportStore(),
      { savedUsername: 'alice', onUsernameSaved },
    )
    controller.setUsername('bob')
    await controller.start()
    expect(search).not.toHaveBeenCalled()
    expect(controller.store.getState().username).toBe('bob')
    await controller.search()
    expect(onUsernameSaved).not.toHaveBeenCalled()
    await controller.dispose()
  })

  it('passes only the selected PGN to the existing preview and deduplicates older pages', async () => {
    const search = vi.fn((_username: string, _cursor: unknown) =>
      Effect.succeed(page),
    )
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
    )
    controller.setUsername(' Alice ')
    await controller.search()
    expect(search).toHaveBeenCalledWith('alice', null)
    expect(controller.selectedPgn()).toBeNull()
    controller.select(game.url)
    expect(controller.selectedPgn()).toBe(game.pgn)
    const preview = parsePgn(controller.selectedPgn() ?? '')
    expect(preview.ok).toBe(true)
    if (preview.ok) {
      expect(preview.meta.white).toBe('PGN Alice')
      expect(preview.meta.whiteElo).toBe('1300')
    }
    await controller.search(true)
    expect(search).toHaveBeenLastCalledWith('alice', page.nextCursor)
    expect(controller.store.getState().items).toHaveLength(1)
    await controller.dispose()
  })

  it('waits for previous cleanup, suppresses stale completion, and cancels without error', async () => {
    let resolveOld: (value: SearchResult) => void = () => {}
    let release: (() => void) | undefined
    const search = vi.fn((username: string) =>
      username === 'alice'
        ? Effect.acquireUseRelease(
            Effect.void,
            () =>
              Effect.promise(
                () =>
                  new Promise<SearchResult>((resolve) => {
                    resolveOld = resolve
                  }),
              ),
            () =>
              Effect.promise(
                () =>
                  new Promise<void>((resolve) => {
                    release = resolve
                  }),
              ),
          )
        : Effect.succeed({ ...page, items: [], nextCursor: null }),
    )
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
    )
    controller.setUsername('alice')
    const old = controller.search()
    await vi.waitFor(() => expect(search).toHaveBeenCalledOnce())
    controller.setUsername('bob')
    const latest = controller.search()
    resolveOld(page)
    expect(search).toHaveBeenCalledOnce()
    await vi.waitFor(() => expect(release).not.toBeUndefined())
    // Cleanup is an uninterruptible finalizer, so the next acquisition waits.
    await vi.waitFor(() =>
      expect(controller.store.getState().loading).toBe(true),
    )
    release?.()
    await Promise.all([old, latest])
    expect(search).toHaveBeenCalledTimes(2)
    expect(controller.store.getState().items).toEqual([])
    expect(controller.store.getState().error).toBeNull()
    await controller.dispose()
  })

  it('restarts an interrupted initial saved-username search when reactivated', async () => {
    const search = vi
      .fn()
      .mockReturnValueOnce(Effect.never)
      .mockReturnValue(Effect.succeed(page))
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
      createChessComImportStore(),
      { savedUsername: 'Alice' },
    )
    const first = controller.start()
    await vi.waitFor(() => expect(search).toHaveBeenCalledOnce())
    controller.suspend()
    await first
    await controller.start()
    expect(search).toHaveBeenCalledTimes(2)
    expect(controller.store.getState().items).toEqual([game])
    expect(controller.store.getState().loading).toBe(false)
    await controller.dispose()
  })

  it('does not restart an explicitly cancelled or completed saved-username search', async () => {
    const search = vi.fn(() => Effect.never)
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
      createChessComImportStore(),
      { savedUsername: 'alice' },
    )
    const first = controller.start()
    await vi.waitFor(() => expect(search).toHaveBeenCalledOnce())
    controller.cancel()
    await first
    await controller.start()
    expect(search).toHaveBeenCalledOnce()
    await controller.dispose()

    const completedSearch = vi.fn(() => Effect.succeed(page))
    const completed = createChessComImportController(
      Layer.succeed(ChessComSearch, { search: completedSearch }),
      createChessComImportStore(),
      { savedUsername: 'alice' },
    )
    await completed.start()
    completed.suspend()
    await completed.start()
    expect(completedSearch).toHaveBeenCalledOnce()
    await completed.dispose()
  })

  it('lets user interaction replace the automatic search without restarting it', async () => {
    const search = vi.fn(() => Effect.never)
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
      createChessComImportStore(),
      { savedUsername: 'alice' },
    )
    const first = controller.start()
    await vi.waitFor(() => expect(search).toHaveBeenCalledOnce())
    controller.setUsername('bob')
    await first
    await controller.start()
    expect(search).toHaveBeenCalledOnce()
    expect(controller.store.getState().username).toBe('bob')
    await controller.dispose()
  })

  it('ignores late results from a suspended search and waits for its teardown', async () => {
    let resolveOld: (value: SearchResult) => void = () => {}
    let finishCleanup: () => void = () => {}
    let cleanupStarted: () => void = () => {}
    const oldResult = new Promise<SearchResult>((resolve) => {
      resolveOld = resolve
    })
    const cleanup = new Promise<void>((resolve) => {
      finishCleanup = resolve
    })
    const teardown = new Promise<void>((resolve) => {
      cleanupStarted = resolve
    })
    const oldGame = { ...game, url: 'https://www.chess.com/game/live/old' }
    const newGame = { ...game, url: 'https://www.chess.com/game/live/new' }
    const search = vi
      .fn()
      .mockImplementationOnce(() =>
        Effect.uninterruptible(Effect.promise(() => oldResult)).pipe(
          Effect.ensuring(
            Effect.gen(function* () {
              cleanupStarted()
              yield* Effect.promise(() => cleanup)
            }),
          ),
        ),
      )
      .mockImplementation(() =>
        Effect.succeed({
          ...page,
          items: [newGame],
          nextCursor: null,
        }),
      )
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
      createChessComImportStore(),
      { savedUsername: 'alice' },
    )
    const first = controller.start()
    await vi.waitFor(() => expect(search).toHaveBeenCalledOnce())
    controller.suspend()
    const restarted = controller.start()
    resolveOld({ ...page, items: [oldGame], nextCursor: null })
    await teardown
    expect(search).toHaveBeenCalledOnce()
    expect(controller.store.getState().loading).toBe(true)
    finishCleanup()
    await Promise.all([first, restarted])
    expect(search).toHaveBeenCalledTimes(2)
    expect(controller.store.getState().items).toEqual([newGame])
    await controller.dispose()
  })

  it('validates usernames locally and preserves earlier results on pagination failure', async () => {
    const search = vi
      .fn()
      .mockReturnValueOnce(Effect.succeed(page))
      .mockReturnValueOnce(
        Effect.fail(
          new ChessComImportError({
            code: 'network',
            message: 'Sem conexão.',
            retryAfterSeconds: null,
          }),
        ),
      )
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
    )
    controller.setUsername('../alice')
    await controller.search()
    expect(search).not.toHaveBeenCalled()
    expect(controller.store.getState().error).toContain('usuário')
    controller.setUsername('alice')
    await controller.search()
    await controller.search(true)
    expect(controller.store.getState().items).toEqual([game])
    expect(controller.store.getState().cursor).toEqual(page.nextCursor)
    expect(controller.store.getState().error).toBe('Sem conexão.')
    await controller.dispose()
  })

  it('does not publish or start further searches after disposal', async () => {
    const search = vi.fn(() => Effect.never)
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
    )
    controller.setUsername('alice')
    const pending = controller.search()
    await vi.waitFor(() => expect(search).toHaveBeenCalledOnce())
    await controller.dispose()
    await pending
    await controller.search()
    expect(search).toHaveBeenCalledOnce()
    expect(controller.store.getState().error).toBeNull()
  })

  it('keeps partial results and rate-limit cooldown across username changes', async () => {
    const search = vi.fn(() =>
      Effect.succeed({
        ...page,
        stopReason: 'error' as const,
        warning: {
          code: 'rateLimited' as const,
          message: 'Aguarde.',
          retryAfterSeconds: 60,
        },
      }),
    )
    const controller = createChessComImportController(
      Layer.succeed(ChessComSearch, { search }),
    )
    controller.setUsername('alice')
    await controller.search()
    expect(controller.store.getState().items).toEqual([game])
    expect(controller.store.getState().coolingDown).toBe(true)
    await controller.search(true)
    controller.setUsername('bob')
    await controller.search()
    expect(search).toHaveBeenCalledOnce()
    await controller.dispose()
  })
})
