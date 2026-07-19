import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }))

import { createTauriEnginePort, type TauriEnginePort } from './engine-port'

type LineHandler = (event: { payload: { id: string; line: string } }) => void

beforeEach(() => {
  mocks.invoke.mockReset()
  mocks.listen.mockReset()
  mocks.invoke.mockResolvedValue(undefined)
})

function wireListen() {
  let lineCb: LineHandler | undefined
  mocks.listen.mockImplementation(async (_event, cb) => {
    lineCb = cb as LineHandler
    return () => {}
  })
  return {
    emit(id: string, line: string) {
      lineCb?.({ payload: { id, line } })
    },
  }
}

function requirePort(p: TauriEnginePort | null): TauriEnginePort {
  if (!p) throw new Error('porta não deveria ser nula')
  return p
}

describe('createTauriEnginePort', () => {
  it('forwards only the lines whose payload id matches the port id', async () => {
    const bus = wireListen()
    const port = requirePort(
      await createTauriEnginePort('primary', undefined, undefined, () => false),
    )

    const received: string[] = []
    port.onLine((line) => received.push(line))

    bus.emit('primary', 'uciok')
    bus.emit('live-wide', 'info depth 10')
    bus.emit('primary', 'readyok')

    expect(received).toEqual(['uciok', 'readyok'])
  })

  it('send routes through engineSend with the port id', async () => {
    wireListen()
    const port = requirePort(
      await createTauriEnginePort(
        'live-wide',
        undefined,
        undefined,
        () => false,
      ),
    )
    await port.send('go infinite')
    expect(mocks.invoke).toHaveBeenCalledWith('engine_send', {
      id: 'live-wide',
      line: 'go infinite',
    })
  })

  it('dispose stops only this port id, leaving other engines alive', async () => {
    wireListen()
    const port = requirePort(
      await createTauriEnginePort(
        'live-wide',
        undefined,
        undefined,
        () => false,
      ),
    )
    await port.dispose()
    expect(mocks.invoke).toHaveBeenCalledWith('engine_stop', {
      id: 'live-wide',
    })
  })
})
