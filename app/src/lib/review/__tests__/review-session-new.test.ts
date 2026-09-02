import { describe, expect, it } from 'vitest'
import {
  depthConfig,
  fakeBackend,
  fakeEnginePort,
  startSession,
} from './review-session-test-helpers'

describe('createReviewSession — análise nova', () => {
  it('start analisa a partida, instala o resultado no store e persiste', async () => {
    const port = fakeEnginePort()
    const { session, store, states } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })

    await session.start()

    expect(store.getSnapshot().result?.moves).toHaveLength(2)
    expect(store.getSnapshot().currentPly).toBe(2)
    expect(states.at(-1)?.status).toBe('done')
  })

  it('repassa progresso rico a cada posição analisada (sem coalescing)', async () => {
    const port = fakeEnginePort()
    const { session, onProgress } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })

    await session.start()

    expect(onProgress).toHaveBeenCalledTimes(5)
    expect(
      onProgress.mock.calls
        .slice(1, 4)
        .map(([progress]) => progress.winPcts.length),
    ).toEqual([1, 2, 3])
    expect(
      onProgress.mock.calls[4][0].winPcts.every(
        (w: number) => Math.abs(w - 50) < 0.1,
      ),
    ).toBe(true)
    expect(onProgress.mock.calls[0][0].stage).toBe('preparing')
    expect(onProgress.mock.calls[1][0]).toMatchObject({
      stage: 'analyzing',
      completed: 1,
      total: 3,
      phase: 'opening',
    })
  })

  it('persiste a revisão exatamente uma vez e encerra a engine (quit)', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    const config = depthConfig()
    const { session } = startSession({ config, backend })

    await session.start()

    expect(backend.saveReview).toHaveBeenCalledTimes(1)
    const [savedConfig, savedResult] = backend.saveReview.mock.calls[0]
    expect(savedConfig).toBe(config)
    expect(savedResult.moves).toHaveLength(2)
    expect(port.sent).toContain('quit')
    expect(port.sent).toContain('__disposed__')
  })

  it('modo automático faz triagem MultiPV sem repassar posições estáveis', async () => {
    const port = fakeEnginePort()
    const { session } = startSession({
      config: depthConfig({
        mode: 'time',
        analysisKind: 'auto-fast',
        lines: 2,
      }),
      backend: fakeBackend(port),
    })

    await session.start()

    expect(port.sent).toContain('setoption name Multipv value 2')
    expect(
      port.sent.filter((command) => command === 'go movetime 120'),
    ).toHaveLength(3)
    expect(port.sent.some((command) => command === 'go movetime 1500')).toBe(
      false,
    )
  })
})
