import { describe, it, expect, vi, beforeEach } from 'vitest'

// hoisted so the mocked modules can reference them without TDZ issues.
const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

import { probeEngine, engineStart, engineSend, engineStop } from './engine'

type LineHandler = (event: { payload: string }) => void

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.listen.mockReset()
})

describe('engine thin wrappers', () => {
  it('engineStart forwards null path when omitted', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineStart()
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', { path: null })
  })

  it('engineStart trims a custom path', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineStart('  /usr/bin/stockfish  ')
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', {
      path: '/usr/bin/stockfish',
    })
  })

  it('engineSend / engineStop forward their commands', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    await engineSend('go depth 20')
    await engineStop()
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', {
      line: 'go depth 20',
    })
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop')
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
      async (cmd: string, args?: { line?: string }) => {
        if (cmd === 'engine_send' && args?.line === 'uci') {
          setTimeout(() => {
            lineCb?.({ payload: 'id name Stockfish 18' })
            lineCb?.({ payload: 'uciok' })
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
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', { line: 'uci' })
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', { path: null })
    // cleanup: engine is stopped at the end.
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop')
  })

  it('reports failure when uciok never arrives (timeout)', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    mocks.listen.mockResolvedValue(() => {})

    const res = await probeEngine(undefined, { timeoutMs: 25 })

    expect(res.ok).toBe(false)
    expect(res.error).toMatch(/Tempo esgotado/i)
    // it still tried to talk to the engine
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', { line: 'uci' })
  })

  it('reports failure when the engine fails to spawn', async () => {
    mocks.listen.mockResolvedValue(() => {})
    mocks.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'engine_spawn') throw new Error('spawn boom')
      return undefined
    })

    const res = await probeEngine(undefined, { timeoutMs: 500 })

    expect(res.ok).toBe(false)
    expect(res.error).toBe('spawn boom')
  })

  it('forwards a custom path to engine_spawn', async () => {
    wireHappyReply()
    const res = await probeEngine('/opt/stockfish', { timeoutMs: 2000 })
    expect(res.ok).toBe(true)
    expect(mocks.invoke).toHaveBeenCalledWith('engine_spawn', {
      path: '/opt/stockfish',
    })
  })
})
