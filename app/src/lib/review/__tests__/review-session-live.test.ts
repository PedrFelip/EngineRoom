import { describe, expect, it, vi } from 'vitest'
import type { PositionCache } from '../../analyze'
import type { LiveAnalysisSettings } from '../../review-session'
import {
  depthConfig,
  existingResult,
  fakeBackend,
  fakeEnginePort,
  startSession,
} from './review-session-test-helpers'

const LIVE_SETTINGS: LiveAnalysisSettings = {
  searchSeconds: 2,
  lines: 3,
  threadsAuto: false,
  threads: 2,
  memoryMb: 64,
  moveFeedbackEnabled: true,
}

describe('createReviewSession — análise ao vivo', () => {
  it.each([
    { incompatible: false, fastPass: false },
    { incompatible: true, fastPass: false },
    { incompatible: false, fastPass: true },
    { incompatible: true, fastPass: true },
  ])(
    'trata pai ausente ou incompatível: %j',
    async ({ incompatible, fastPass }) => {
      const port = fakeEnginePort()
      const result = existingResult()
      const { session, store } = startSession({
        config: depthConfig({ initialResult: result }),
        backend: fakeBackend(port),
      })
      await session.start()
      store.first()
      store.exploreLine(['d2d4', 'd7d5'])
      store.next()
      const parent = store.getSnapshot().variation?.roots[0]
      const move = parent?.children[0]
      if (!parent || !move) throw new Error('Variação de dois lances esperada')
      const classify = vi.spyOn(store, 'setVariationClassification')

      try {
        session.analyzePosition(
          {
            fen: move.fen,
            variationNodeId: move.id,
            sourceFen: parent.fen,
            sourceAnalysis: incompatible
              ? { ...result.positions[0], cp: 2000 }
              : undefined,
          },
          { ...LIVE_SETTINGS, fastPass },
        )
        await vi.waitFor(() => {
          expect(
            store.getSnapshot().liveAnalysis.positions[move.fen],
          ).toBeDefined()
          if (!fastPass)
            expect(classify).toHaveBeenCalledWith(move.id, 'melhor')
        })
        expect(port.sent).toContain(`position fen ${move.fen}`)
        if (fastPass) {
          expect(classify).not.toHaveBeenCalled()
          expect(port.sent).not.toContain(`position fen ${parent.fen}`)
        } else {
          expect(port.sent).toContain(`position fen ${parent.fen}`)
        }
      } finally {
        session.dispose()
      }
    },
  )

  it('analisa a posição atual com tempo, MultiPV e recursos configurados', async () => {
    const port = fakeEnginePort()
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend: fakeBackend(port),
    })
    await session.start()
    const fen = existingResult().positions[0].fen

    session.analyzePosition({ fen }, LIVE_SETTINGS)

    await vi.waitFor(() => {
      expect(store.getSnapshot().liveAnalysis.positions[fen]).toBeDefined()
    })
    expect(port.sent).toContain('setoption name Threads value 2')
    expect(port.sent).toContain('setoption name Hash value 64')
    expect(port.sent).toContain('setoption name Multipv value 3')
    expect(port.sent).toContain('go movetime 2000')
  })

  it('faz uma passada curta durante a reprodução e classifica com a posição anterior', async () => {
    const port = fakeEnginePort()
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend: fakeBackend(port),
    })
    await session.start()
    store.goTo(0)
    store.makeMove('d2', 'd4')
    const move = store.getSnapshot().variation?.roots[0]
    const source = existingResult().positions[0]

    session.analyzePosition(
      {
        fen: move?.fen ?? '',
        variationNodeId: move?.id,
        sourceFen: source.fen,
        sourceAnalysis: source,
      },
      { ...LIVE_SETTINGS, fastPass: true },
    )

    await vi.waitFor(() => {
      expect(store.getSnapshot().variation?.roots[0]?.classification).toBe(
        'melhor',
      )
    })
    expect(port.sent).toContain('setoption name Multipv value 1')
    expect(port.sent).toContain('go movetime 500')
  })

  it('classifica o lance novo depois de analisar sua posição final', async () => {
    const port = fakeEnginePort()
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend: fakeBackend(port),
    })
    await session.start()
    store.goTo(0)
    store.makeMove('d2', 'd4')
    const variation = store.getSnapshot().variation
    const move = variation?.roots[0]
    const source = existingResult().positions[0]
    expect(move).toBeDefined()

    session.analyzePosition(
      {
        fen: move?.fen ?? '',
        variationNodeId: move?.id,
        sourceFen: source.fen,
        sourceAnalysis: source,
      },
      LIVE_SETTINGS,
    )

    await vi.waitFor(() => {
      expect(store.getSnapshot().variation?.roots[0]?.classification).toBe(
        'melhor',
      )
    })
  })

  it('dimensiona threads e hash pelos recursos detectados no modo automático', async () => {
    const port = fakeEnginePort()
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend: fakeBackend(port),
    })
    await session.start()
    const fen = existingResult().positions[0].fen

    session.analyzePosition({ fen }, { ...LIVE_SETTINGS, threadsAuto: true })

    await vi.waitFor(() => {
      expect(store.getSnapshot().liveAnalysis.positions[fen]).toBeDefined()
    })
    expect(port.sent).toContain('setoption name Threads value 2')
    expect(port.sent).toContain('setoption name Hash value 1638')
  })

  it('usa os valores manuais se a detecção automática falhar', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    backend.getSystemResources = async () => {
      throw new Error('recursos indisponíveis')
    }
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend,
    })
    await session.start()
    const fen = existingResult().positions[0].fen

    session.analyzePosition({ fen }, { ...LIVE_SETTINGS, threadsAuto: true })

    await vi.waitFor(() => {
      expect(store.getSnapshot().liveAnalysis.positions[fen]).toBeDefined()
    })
    expect(port.sent).toContain('setoption name Threads value 2')
    expect(port.sent).toContain('setoption name Hash value 64')
  })

  it('não classifica o lance quando o feedback está desligado', async () => {
    const port = fakeEnginePort()
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend: fakeBackend(port),
    })
    await session.start()
    store.goTo(0)
    store.makeMove('d2', 'd4')
    const move = store.getSnapshot().variation?.roots[0]

    session.analyzePosition(
      {
        fen: move?.fen ?? '',
        variationNodeId: move?.id,
        sourceFen: existingResult().positions[0].fen,
        sourceAnalysis: existingResult().positions[0],
      },
      { ...LIVE_SETTINGS, moveFeedbackEnabled: false },
    )

    await vi.waitFor(() => {
      expect(
        store.getSnapshot().liveAnalysis.positions[move?.fen ?? ''],
      ).toBeDefined()
    })
    expect(
      store.getSnapshot().variation?.roots[0]?.classification,
    ).toBeUndefined()
  })

  it('descarta uma posição substituída antes de iniciar sua busca', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    let releaseFirstLookup = () => {}
    const firstLookup = new Promise<void>((resolve) => {
      releaseFirstLookup = resolve
    })
    let lookupCount = 0
    const cache: PositionCache = {
      async get() {
        lookupCount++
        if (lookupCount === 1) await firstLookup
        return null
      },
      async put() {},
      async getBulk(fens) {
        return fens.map(() => null)
      },
      async putMany() {},
    }
    backend.createPositionCache = () => cache
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend,
    })
    await session.start()
    const [first, second] = existingResult().positions

    session.analyzePosition({ fen: first.fen }, LIVE_SETTINGS)
    await vi.waitFor(() => {
      expect(store.getSnapshot().liveAnalysis.status).toBe('running')
    })
    session.analyzePosition({ fen: second.fen }, LIVE_SETTINGS)
    releaseFirstLookup()

    await vi.waitFor(() => {
      expect(
        store.getSnapshot().liveAnalysis.positions[second.fen],
      ).toBeDefined()
    })
    expect(port.sent).not.toContain(`position fen ${first.fen}`)
    expect(port.sent).toContain(`position fen ${second.fen}`)
  })
})
