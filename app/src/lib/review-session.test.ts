import { describe, expect, it, vi } from 'vitest'
import type { ReviewConfig } from '../types'
import type { PositionCache } from './analyze'
import type { Backend, EnginePortHandle } from './backend'
import { createReviewSession, type ReviewSessionState } from './review-session'
import { createReviewStore } from './review-store'

/** Port que responde o handshake UCI e cada `go` com info + bestmove.
 * `emit` injeta uma linha UCI como se viesse do stdout da engine. */
function fakeEnginePort(): EnginePortHandle & {
  sent: string[]
  emit(line: string): void
} {
  const sent: string[] = []
  let lineCb: ((line: string) => void) | null = null
  return {
    sent,
    send(cmd: string) {
      sent.push(cmd.trim())
      const c = cmd.trim()
      if (c === 'uci') lineCb?.('uciok')
      else if (c === 'isready') lineCb?.('readyok')
      else if (c.startsWith('go') && !c.startsWith('go infinite')) {
        lineCb?.('info depth 20 multipv 1 score cp 0 pv e2e4 e7e5')
        lineCb?.('bestmove e2e4')
      }
    },
    onLine(handler: (line: string) => void) {
      lineCb = handler
      return () => {
        lineCb = null
      }
    },
    emit(line: string) {
      lineCb?.(line)
    },
    async dispose() {
      sent.push('__disposed__')
    },
  }
}

function missCache(): PositionCache {
  return {
    async get() {
      return null
    },
    async put() {},
    async getBulk(fens) {
      return fens.map(() => null)
    },
    async putMany() {},
  }
}

function fakeBackend(
  port: EnginePortHandle,
): Backend & { saveReview: ReturnType<typeof vi.fn> } {
  return {
    createEnginePort: async () => port,
    getSystemResources: async () => ({ threads: 4, memory_mb: 8192 }),
    createPositionCache: () => missCache(),
    saveReview: vi.fn(async () => 1),
  }
}

function depthConfig(overrides: Partial<ReviewConfig> = {}): ReviewConfig {
  return {
    pgn: '1. e4 e5',
    meta: {
      white: 'W',
      black: 'B',
      whiteElo: null,
      blackElo: null,
      result: '1-0',
      event: null,
      plies: 2,
    },
    engine: { id: 'balanced', label: 'Equilibrado', depth: 20, hint: '' },
    mode: 'depth',
    lines: 1,
    ...overrides,
  }
}

/** Revisão pré-existente (vinda do store) com FENs conhecidas de '1. e4 e5'. */
function existingResult() {
  const fens = [
    'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
    'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1',
    'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2',
  ]
  return {
    positions: fens.map((fen, i) => ({
      ply: i,
      fen,
      phase: 'opening' as const,
      depth: 20,
      cp: 0,
      winPct: 50,
      pv: ['e2e4'],
      lines: [],
    })),
    moves: [
      {
        ply: 1,
        color: 'w' as const,
        san: 'e4',
        uci: 'e2e4',
        fenBefore: fens[0],
        classification: 'melhor' as const,
        winPctBefore: 50,
        winPctAfter: 50,
        winPctLoss: 0,
        cpLoss: 0,
        bestUci: 'e2e4',
        isBook: false,
        eco: null,
      },
      {
        ply: 2,
        color: 'b' as const,
        san: 'e5',
        uci: 'e7e5',
        fenBefore: fens[1],
        classification: 'melhor' as const,
        winPctBefore: 50,
        winPctAfter: 50,
        winPctLoss: 0,
        cpLoss: 0,
        bestUci: 'e7e5',
        isBook: false,
        eco: null,
      },
    ],
    accuracy: { white: 100, black: 100 },
    accuracyByPhase: {
      opening: { white: 100, black: 100 },
      middlegame: { white: 100, black: 100 },
      endgame: { white: 100, black: 100 },
    },
  }
}

function startSession(opts: {
  config: ReviewConfig
  backend: Backend
  store?: ReturnType<typeof createReviewStore>
}) {
  const states: ReviewSessionState[] = []
  const store = opts.store ?? createReviewStore()
  const onProgress = vi.fn()
  const session = createReviewSession({
    config: opts.config,
    enginePath: undefined,
    backend: opts.backend,
    store,
    onStateChange: (s) => states.push(s),
    onProgress,
  })
  return { session, store, states, onProgress }
}

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
      enginePath: undefined,
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
