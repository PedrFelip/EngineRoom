import { createElement } from 'react'
import { act, create, type ReactTestRenderer } from 'react-test-renderer'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { type HistoryIO, useHistoryController } from './use-history-controller'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}
type History = ReturnType<typeof useHistoryController>
type Config = NonNullable<Awaited<ReturnType<HistoryIO['open']>>>
type StoredGame = NonNullable<Awaited<ReturnType<HistoryIO['get']>>>
const config: Config = {
  pgn: '1. e4 *',
  meta: {
    white: 'A',
    black: 'B',
    whiteElo: null,
    blackElo: null,
    result: '*',
    event: null,
    plies: 1,
  },
  engine: { id: 'fast', label: 'Rápido', depth: 15, hint: '' },
  mode: 'depth',
  lines: 1,
}
const game: StoredGame = {
  id: 1,
  white: 'A',
  black: 'B',
  result: '*',
  plies: 1,
  engineTier: 'fast',
  mode: 'depth',
  depth: 15,
  multipv: 1,
  accuracyWhite: 0,
  accuracyBlack: 0,
  createdAt: '2026-01-01',
  pgn: config.pgn,
  reviewJson: '{}',
}
const page = { games: [game], total: 1, nextCursor: null }

function Test({
  options,
  capture,
}: {
  options: Parameters<typeof useHistoryController>[0]
  capture: (history: History) => void
}) {
  const history = useHistoryController(options)
  capture(history)
  return null
}

function harness(io: HistoryIO) {
  let history!: History
  const onStart = vi.fn()
  const importPgn = vi.fn()
  const setView = vi.fn()
  const setStep = vi.fn()
  const capture = (next: History) => {
    history = next
  }
  const render = (active: boolean) =>
    createElement(Test, {
      options: { active, io, onStart, importPgn, setView, setStep },
      capture,
    })
  let renderer!: ReactTestRenderer
  return {
    onStart,
    importPgn,
    setView,
    setStep,
    current: () => history,
    mount: async () => {
      await act(async () => {
        renderer = create(render(true))
      })
    },
    setActive: async (active: boolean) => {
      await act(async () => {
        renderer.update(render(active))
      })
    },
    unmount: async () => {
      await act(async () => {
        renderer.unmount()
      })
    },
  }
}
function ioDefaults(): HistoryIO {
  return {
    list: vi.fn().mockResolvedValue(page),
    open: vi.fn().mockResolvedValue(config),
    get: vi.fn().mockResolvedValue(game),
    delete: vi.fn().mockResolvedValue(undefined),
  }
}

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('window', { scrollTo: vi.fn() })
  vi.spyOn(console, 'error').mockImplementation((...args) => {
    if (String(args[0]).includes('react-test-renderer is deprecated')) return
    throw new Error(args.map(String).join(' '))
  })
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('history lifecycle', () => {
  it('ignores a cancelled open and its cleanup after a newer open starts', async () => {
    const first = deferred<Config | null>()
    const second = deferred<Config | null>()
    const io = ioDefaults()
    io.open = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const h = harness(io)
    await h.mount()
    await act(async () => {
      h.current().openStored(1)
    })
    const firstSignal = vi.mocked(io.open).mock.calls[0][1]
    await h.setActive(false)
    expect(firstSignal.aborted).toBe(true)
    await h.setActive(true)
    await act(async () => {
      h.current().openStored(2)
    })
    await act(async () => {
      first.resolve(config)
      await first.promise
    })
    expect(h.onStart).not.toHaveBeenCalled()
    expect(h.current().pendingGame).toBe(2)
    await act(async () => {
      second.resolve(config)
      await second.promise
    })
    expect(h.onStart).toHaveBeenCalledTimes(1)
    expect(h.current().pendingGame).toBeNull()
    await h.unmount()
  })

  it('does not navigate when a cancelled reanalysis resolves after unmount', async () => {
    const request = deferred<StoredGame | null>()
    const io = ioDefaults()
    io.get = vi.fn().mockReturnValue(request.promise)
    const h = harness(io)
    await h.mount()
    await act(async () => {
      h.current().reanalyzeStored(1)
    })
    const signal = vi.mocked(io.get).mock.calls[0][1]
    await h.unmount()
    expect(signal.aborted).toBe(true)
    await act(async () => {
      request.resolve(game)
      await request.promise
    })
    expect(h.importPgn).not.toHaveBeenCalled()
    expect(h.setView).not.toHaveBeenCalled()
    expect(h.setStep).not.toHaveBeenCalled()
    expect(window.scrollTo).not.toHaveBeenCalled()
  })

  it('serializes deletion through deactivation and reloads after it completes', async () => {
    const deletion = deferred<void>()
    const io = ioDefaults()
    io.delete = vi.fn().mockReturnValue(deletion.promise)
    const h = harness(io)
    await h.mount()
    let operation!: Promise<boolean>
    await act(async () => {
      operation = h.current().removeStored(1)
    })
    await h.setActive(false)
    await h.setActive(true)
    await act(async () => {
      expect(await h.current().removeStored(2)).toBe(false)
      h.current().openStored(2)
    })
    expect(io.delete).toHaveBeenCalledTimes(1)
    expect(io.open).not.toHaveBeenCalled()
    const callsBeforeCompletion = vi.mocked(io.list).mock.calls.length
    await act(async () => {
      deletion.resolve()
      expect(await operation).toBe(true)
    })
    expect(io.list).toHaveBeenCalledTimes(callsBeforeCompletion + 1)
    await h.unmount()
  })
})
