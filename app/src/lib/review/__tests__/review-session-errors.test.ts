import { describe, expect, it } from 'vitest'
import {
  depthConfig,
  fakeBackend,
  fakeEnginePort,
  startSession,
} from './review-session-test-helpers'

describe('createReviewSession — falhas', () => {
  /** Port que responde handshake mas morre (sync) no primeiro `go`. */
  function portDyingOnGo(): EnginePortHandle & { sent: string[] } {
    const sent: string[] = []
    let lineCb: ((line: string) => void) | null = null
    return {
      sent,
      send(cmd: string) {
        sent.push(cmd.trim())
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go depth')) {
          throw new Error('engine morreu no go')
        }
      },
      onLine(handler: (line: string) => void) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
      async dispose() {
        sent.push('__disposed__')
      },
    }
  }

  it('erro da engine vira status error com a mensagem original, sem persistir', async () => {
    const port = portDyingOnGo()
    const backend = fakeBackend(port)
    const { session, store, states } = startSession({
      config: depthConfig(),
      backend,
    })

    await session.start()

    expect(states.at(-1)).toEqual({
      status: 'error',
      error: 'engine morreu no go',
    })
    expect(store.getSnapshot().result).toBeNull()
    expect(backend.saveReview).not.toHaveBeenCalled()
  })

  it('falha de sizing é engolida: análise segue com defaults (sem setoption Threads)', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    backend.getSystemResources = async () => {
      throw new Error('sem /proc')
    }
    const { session, store } = startSession({ config: depthConfig(), backend })

    await session.start()

    expect(store.getSnapshot().result).not.toBeNull()
    expect(port.sent).not.toContain('setoption name Threads value 4')
  })
})
