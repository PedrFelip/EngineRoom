import { describe, expect, it } from 'vitest'
import {
  depthConfig,
  fakeBackend,
  fakeEnginePort,
  startSession,
} from './review-session-test-helpers'

describe('createReviewSession — descarte e cancelamento', () => {
  const tick = () => new Promise<void>((r) => setTimeout(r, 0))

  it('dispose após análise concluída encerra a engine', async () => {
    const port = fakeEnginePort()
    const { session } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })
    await session.start()

    session.dispose()
    await tick()

    // após start, port já foi disposto via finally (quit + disposed)
    expect(port.sent).toContain('quit')
    expect(port.sent).toContain('__disposed__')
  })

  it('boot abortado (isCancelled) não analisa nem encerra porta inexistente', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    // Factory fiel ao contrato: devolve null se cancelado durante o boot.
    backend.createEnginePort = async (_path, isCancelled) => {
      await tick()
      return isCancelled() ? null : port
    }
    const { session, store, states } = startSession({
      config: depthConfig(),
      backend,
    })
    const startPromise = session.start()

    session.dispose() // cancela antes do spawn concluir
    await startPromise

    expect(store.getSnapshot().result).toBeNull()
    expect(port.sent).toEqual([])
    expect(states).toEqual([])
  })

  it('porta viva na janela de cancelamento é encerrada (sem engine órfã)', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    // Factory que ignora isCancelled: devolve a porta mesmo cancelada.
    backend.createEnginePort = async () => {
      await tick()
      return port
    }
    const { session } = startSession({ config: depthConfig(), backend })
    const startPromise = session.start()

    session.dispose()
    await startPromise
    await tick()

    expect(port.sent).toContain('__disposed__')
    expect(port.sent).not.toContain('uci')
  })
})
