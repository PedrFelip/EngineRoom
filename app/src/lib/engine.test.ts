import { Deferred, Effect, Fiber } from 'effect'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { runUi } from './__tests__/reference/effect/ui-runtime'

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

import { probeEngine } from './__tests__/reference/engine'
import {
  createTauriEnginePort,
  ENGINE_LINE_EVENT,
} from './__tests__/reference/engine-port'

beforeEach(() => {
  mocks.invoke.mockReset().mockResolvedValue(undefined)
  mocks.listen.mockReset().mockResolvedValue(() => {})
})

describe('scoped engine and probe', () => {
  it('acquires, sends commands and stops exactly once', async () => {
    const off = vi.fn()
    mocks.listen.mockResolvedValue(off)
    await runUi(
      Effect.scoped(
        Effect.gen(function* () {
          const port = yield* createTauriEnginePort
          yield* port.send('go depth 20')
        }),
      ),
    )
    expect(mocks.invoke.mock.calls).toEqual([
      ['engine_spawn', undefined],
      ['engine_send', { line: 'go depth 20' }],
      ['engine_stop', undefined],
    ])
    expect(off).toHaveBeenCalledTimes(2)
  })

  it('envia um lote UCI em uma única chamada IPC', async () => {
    await runUi(
      Effect.scoped(
        Effect.gen(function* () {
          const port = yield* createTauriEnginePort
          if (!port.sendBatch) throw new Error('batch UCI indisponível')
          yield* port.sendBatch(['position fen test', 'go depth 20'])
        }),
      ),
    )

    expect(mocks.invoke).toHaveBeenCalledWith('engine_send_batch', {
      lines: ['position fen test', 'go depth 20'],
    })
  })

  it('sends uci and obtains the name even with a synchronous reply', async () => {
    let line: (event: { payload: unknown }) => void = () => {}
    mocks.listen.mockImplementation(async (event, cb) => {
      if (event === ENGINE_LINE_EVENT) line = cb
      return () => {}
    })
    mocks.invoke.mockImplementation(async (command, args) => {
      if (command === 'engine_send' && args.line === 'uci') {
        line({ payload: 'id name Stockfish 18' })
        line({ payload: 'uciok' })
      }
    })
    expect(await runUi(probeEngine())).toEqual({
      ok: true,
      name: 'Stockfish 18',
    })
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', { line: 'uci' })
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop', undefined)
  })

  it('times out and releases both listeners and process', async () => {
    const off = vi.fn()
    mocks.listen.mockResolvedValue(off)
    const result = await runUi(probeEngine({ timeoutMs: 10 }))
    expect(result.ok).toBe(false)
    expect(result.error).toContain('não respondeu')
    expect(off).toHaveBeenCalledTimes(2)
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop', undefined)
  })

  it('reports a spawn failure without stopping another process', async () => {
    mocks.invoke.mockRejectedValueOnce(new Error('spawn boom'))
    expect(await runUi(probeEngine())).toMatchObject({
      ok: false,
      error: 'spawn boom',
    })
    expect(mocks.listen).toHaveBeenCalledTimes(2)
    expect(mocks.invoke).not.toHaveBeenCalledWith('engine_stop', undefined)
  })

  it.each([1, 2])(
    'releases partial acquisition when listener %i fails',
    async (which) => {
      const off = vi.fn()
      if (which === 2) mocks.listen.mockResolvedValueOnce(off)
      mocks.listen.mockRejectedValueOnce(new Error('listen boom'))
      await expect(runUi(Effect.scoped(createTauriEnginePort))).rejects.toThrow(
        'listen boom',
      )
      expect(off).toHaveBeenCalledTimes(which - 1)
      expect(mocks.invoke).not.toHaveBeenCalled()
    },
  )

  it('waits for an in-flight spawn before completing interruption and cleanup', async () => {
    const off = vi.fn()
    mocks.listen.mockResolvedValue(off)
    let finish = () => {}
    const spawning = new Promise<void>((resolve) => {
      finish = resolve
    })
    mocks.invoke.mockImplementation((command) =>
      command === 'engine_spawn' ? spawning : Promise.resolve(),
    )
    const fiber = Effect.runFork(Effect.scoped(createTauriEnginePort))
    await vi.waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', undefined),
    )
    const interrupted = runUi(Fiber.interrupt(fiber))
    finish()
    await interrupted
    expect(off).toHaveBeenCalledTimes(2)
    expect(
      mocks.invoke.mock.calls.filter(([c]) => c === 'engine_stop'),
    ).toHaveLength(1)
  })

  it('serializes owners and does not spawn an interrupted waiter', async () => {
    const ready = Effect.runSync(Deferred.make<void>())
    const owner = Effect.runFork(
      Effect.scoped(
        Effect.gen(function* () {
          yield* createTauriEnginePort
          yield* Deferred.succeed(ready, undefined)
          yield* Effect.never
        }),
      ),
    )
    await runUi(Deferred.await(ready))
    const waiter = Effect.runFork(Effect.scoped(createTauriEnginePort))
    try {
      await runUi(Effect.yieldNow())
      await runUi(Fiber.interrupt(waiter))
      expect(
        mocks.invoke.mock.calls.filter(([c]) => c === 'engine_spawn'),
      ).toHaveLength(1)
      expect(mocks.invoke).not.toHaveBeenCalledWith('engine_stop', undefined)
    } finally {
      await runUi(Fiber.interrupt(owner))
    }
    await runUi(Effect.scoped(createTauriEnginePort))
    expect(
      mocks.invoke.mock.calls.filter(([c]) => c === 'engine_spawn'),
    ).toHaveLength(2)
    expect(
      mocks.invoke.mock.calls.filter(([c]) => c === 'engine_stop'),
    ).toHaveLength(2)
  })

  it('retains an exit received during spawn, before the handshake subscribes', async () => {
    let exit: (event: { payload: unknown }) => void = () => {}
    mocks.listen.mockImplementation(async (event, cb) => {
      if (event === 'engine://exit') exit = cb
      return () => {}
    })
    mocks.invoke.mockImplementation(async (command) => {
      if (command === 'engine_spawn') {
        exit({ payload: { code: null, signal: 11, error: null } })
      }
    })
    const result = await runUi(probeEngine())
    expect(result).toMatchObject({ ok: false })
    expect(result.error).toContain('sinal 11')
    expect(mocks.invoke).not.toHaveBeenCalledWith('engine_send', {
      line: 'uci',
    })
  })
})
