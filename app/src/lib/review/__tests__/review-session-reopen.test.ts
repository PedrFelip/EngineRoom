import { describe, expect, it, vi } from 'vitest'
import {
  createReviewSession,
  type ReviewSessionState,
} from '../../review-session'
import { createReviewStore } from '../../review-store'
import {
  depthConfig,
  existingResult,
  fakeBackend,
  fakeEnginePort,
  startSession,
} from './review-session-test-helpers'

describe('createReviewSession — reabertura do store', () => {
  function reopenBackend() {
    const port = fakeEnginePort()
    return { port, backend: fakeBackend(port) }
  }

  it('instala o initialResult antes de qualquer I/O e não re-analisa nem re-salva', async () => {
    const { port, backend } = reopenBackend()
    const initialResult = existingResult()
    const store = createReviewStore()
    const states: ReviewSessionState[] = []
    const session = createReviewSession({
      config: depthConfig({ initialResult }),
      backend,
      store,
      onStateChange: (s) => states.push(s),
      onProgress: vi.fn(),
    })
    const startPromise = session.start()

    // Síncrono: antes mesmo do start() aguardar algo, o resultado já está no store.
    expect(store.getSnapshot().result).toBe(initialResult)
    expect(states[0]?.status).toBe('done')

    await startPromise
    expect(backend.saveReview).not.toHaveBeenCalled()
    expect(port.sent.some((c) => c.startsWith('go depth'))).toBe(false)
  })

  it('não sobe engine nem faz handshake ao reabrir (sem refino ao vivo)', async () => {
    const { port, backend } = reopenBackend()
    const { session } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend,
    })

    await session.start()

    expect(port.sent).not.toContain('uci')
    expect(port.sent).not.toContain('isready')
    expect(port.sent).not.toContain('go infinite')
  })
})
