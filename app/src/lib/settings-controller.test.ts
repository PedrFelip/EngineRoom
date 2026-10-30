import { describe, expect, it, vi } from 'vitest'
import type { ProbeResult } from './engine'
import {
  createSettingsController,
  type SettingsIO,
} from './settings-controller'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok
    reject = fail
  })
  return { promise, resolve, reject }
}
const flush = async () => {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}
const stats = (bytes: number) => ({
  cacheBytes: bytes,
  gamesBytes: bytes,
  dbBytes: bytes,
})

describe('settings controller lifecycle', () => {
  it('rejects late probe and storage results after close and reopen', async () => {
    const first = deferred<ReturnType<typeof stats>>()
    const second = deferred<ReturnType<typeof stats>>()
    const probe = deferred<ProbeResult>()
    const io: SettingsIO = {
      stats: vi
        .fn()
        .mockReturnValueOnce(first.promise)
        .mockReturnValueOnce(second.promise),
      probe: () => probe.promise,
      clearCache: async () => {},
      clearGames: async () => {},
    }
    const controller = createSettingsController(
      io,
      () => {},
      () => {},
    )
    controller.open()
    controller.test()
    controller.close()
    controller.open()
    second.resolve(stats(2))
    first.resolve(stats(1))
    probe.resolve({ ok: true, name: 'old' })
    await flush()
    expect(controller.store.getState()).toMatchObject({
      stats: stats(2),
      result: null,
      testing: false,
    })
  })

  it('serializes clears across reopening and refreshes after completion', async () => {
    const clearing = deferred<void>()
    const calls = vi.fn(() => clearing.promise)
    const onGamesCleared = vi.fn()
    const finished = vi.fn()
    const io: SettingsIO = {
      stats: vi.fn().mockResolvedValue(stats(4)),
      probe: vi.fn(),
      clearCache: vi.fn(),
      clearGames: calls,
    }
    const controller = createSettingsController(io, onGamesCleared, finished)
    controller.open()
    const operation = controller.clear('games')
    controller.close()
    controller.open()
    await controller.clear('games')
    expect(calls).toHaveBeenCalledTimes(1)
    expect(controller.store.getState().acting).toBe(true)
    clearing.resolve()
    await operation
    await flush()
    expect(onGamesCleared).toHaveBeenCalledTimes(1)
    expect(finished).not.toHaveBeenCalled()
    expect(controller.store.getState()).toMatchObject({
      acting: false,
      stats: stats(4),
    })
  })

  it('prevents an earlier storage read from overwriting a clear refresh', async () => {
    const first = deferred<ReturnType<typeof stats>>()
    const io: SettingsIO = {
      stats: vi
        .fn()
        .mockReturnValueOnce(first.promise)
        .mockResolvedValue(stats(0)),
      probe: vi.fn(),
      clearCache: async () => {},
      clearGames: async () => {},
    }
    const controller = createSettingsController(
      io,
      () => {},
      () => {},
    )
    controller.open()
    await controller.clear('cache')
    first.resolve(stats(100))
    await flush()
    expect(controller.store.getState().stats).toEqual(stats(0))
  })
})
