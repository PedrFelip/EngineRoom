import { beforeEach, describe, expect, it, vi } from 'vitest'

// hoisted so the mocked modules can reference them without TDZ issues.
const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

import {
  engineSend,
  engineStart,
  engineStop,
  onEngineLine,
  probeEngine,
} from './engine'

type LineHandler = (event: { payload: { id: string; line: string } }) => void

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.listen.mockReset()
})

describe('engine thin wrappers (multi-engine)', () => {
  it('engineStart passes id and null path to engine_spawn when path omitted', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineStart('primary')
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', {
      id: 'primary',
      path: null,
    })
  })

  it('engineStart forwards id and a trimmed custom path', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineStart('live-wide', '  /usr/bin/stockfish  ')
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', {
      id: 'live-wide',
      path: '/usr/bin/stockfish',
    })
  })

  it('engineSend routes a line to a specific id', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineSend('live-wide', 'go infinite')
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', {
      id: 'live-wide',
      line: 'go infinite',
    })
  })

  it('engineStop targets a specific id', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineStop('primary')
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop', { id: 'primary' })
  })

  it('onEngineLine forwards id and line extracted from the payload', async () => {
    let lineCb: LineHandler | undefined
    mocks.listen.mockImplementation(async (_event, cb) => {
      lineCb = cb as LineHandler
      return () => {}
    })

    const received: Array<[string, string]> = []
    await onEngineLine((id, line) => received.push([id, line]))

    lineCb?.({ payload: { id: 'primary', line: 'uciok' } })
    lineCb?.({ payload: { id: 'live-wide', line: 'info depth 10' } })

    expect(received).toEqual([
      ['primary', 'uciok'],
      ['live-wide', 'info depth 10'],
    ])
  })
})

describe('probeEngine', () => {
  /** Wires listen/invoke so a `uci` send replies with id name + uciok. */
  function wireHappyReply() {
    let lineCb: LineHandler | undefined
    mocks.listen.mockImplementation(async (_event, cb) => {
      lineCb = cb as LineHandler
      return () => {}
    })
    mocks.invoke.mockImplementation(
      async (cmd: string, args?: { line?: string; id?: string }) => {
        if (
          cmd === 'engine_send' &&
          args?.line === 'uci' &&
          args?.id === 'probe'
        ) {
          setTimeout(() => {
            lineCb?.({ payload: { id: 'probe', line: 'id name Stockfish 18' } })
            lineCb?.({ payload: { id: 'probe', line: 'uciok' } })
          }, 0)
        }
        return undefined
      },
    )
  }

  it('sends `uci`, resolves ok with the engine name on uciok', async () => {
    wireHappyReply()
    const res = await probeEngine(undefined, { timeoutMs: 2000 })

    expect(res.ok).toBe(true)
    expect(res.name).toBe('Stockfish 18')

    // Regression guard: the probe MUST actually issue the `uci` command,
    // otherwise the engine never answers and the probe times out.
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', {
      id: 'probe',
      line: 'uci',
    })
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', {
      id: 'probe',
      path: null,
    })
    // cleanup: engine is stopped at the end.
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop', { id: 'probe' })
  })

  it('reports failure when uciok never arrives (timeout)', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    mocks.listen.mockResolvedValue(() => {})

    const res = await probeEngine(undefined, { timeoutMs: 25 })

    expect(res.ok).toBe(false)
    expect(res.error).toMatch(/Tempo esgotado/i)
    // it still tried to talk to the engine
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', {
      id: 'probe',
      line: 'uci',
    })
  })

  it('reports failure when the engine fails to spawn', async () => {
    mocks.listen.mockResolvedValue(() => {})
    mocks.invoke.mockImplementation(
      async (cmd: string, args?: { id?: string }) => {
        if (cmd === 'engine_spawn' && args?.id === 'probe')
          throw new Error('spawn boom')
        return undefined
      },
    )

    const res = await probeEngine(undefined, { timeoutMs: 500 })

    expect(res.ok).toBe(false)
    expect(res.error).toBe('spawn boom')
  })

  it('forwards a custom path to engine_spawn', async () => {
    wireHappyReply()
    const res = await probeEngine('/opt/stockfish', { timeoutMs: 2000 })
    expect(res.ok).toBe(true)
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', {
      id: 'probe',
      path: '/opt/stockfish',
    })
  })

  it('ignores lines emitted by other engine ids', async () => {
    let lineCb: LineHandler | undefined
    mocks.listen.mockImplementation(async (_event, cb) => {
      lineCb = cb as LineHandler
      return () => {}
    })
    mocks.invoke.mockImplementation(
      async (cmd: string, args?: { line?: string; id?: string }) => {
        if (cmd === 'engine_send' && args?.line === 'uci') {
          setTimeout(() => {
            // Another engine's noise must not satisfy the probe.
            lineCb?.({ payload: { id: 'live-wide', line: 'uciok' } })
            // Then the probe's own uciok.
            lineCb?.({ payload: { id: 'probe', line: 'uciok' } })
          }, 0)
        }
        return undefined
      },
    )

    const res = await probeEngine(undefined, { timeoutMs: 2000 })
    expect(res.ok).toBe(true)
  })
})
