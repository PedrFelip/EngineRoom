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

  it('repassa winPcts crus a cada posição analisada (sem coalescing)', async () => {
    const port = fakeEnginePort()
    const { session, onProgress } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })

    await session.start()

    expect(onProgress).toHaveBeenCalledTimes(3)
    expect(onProgress.mock.calls.map(([wp]) => wp.length)).toEqual([1, 2, 3])
    expect(
      onProgress.mock.calls[2][0].every((w: number) => Math.abs(w - 50) < 0.1),
    ).toBe(true)
  })

  it('persiste a revisão exatamente uma vez e mantém a engine viva (sem quit)', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    const config = depthConfig()
    const { session } = startSession({ config, backend })

    await session.start()

    expect(backend.saveReview).toHaveBeenCalledTimes(1)
    const [savedConfig, savedResult] = backend.saveReview.mock.calls[0]
    expect(savedConfig).toBe(config)
    expect(savedResult.moves).toHaveLength(2)
    expect(port.sent).not.toContain('quit')
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

  it('ainda faz o handshake e sobe o refino ao vivo (go infinite)', async () => {
    const { port, backend } = reopenBackend()
    const { session } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend,
    })

    await session.start()

    expect(port.sent).toContain('uci')
    expect(port.sent).toContain('isready')
    expect(port.sent).toContain('go infinite')
    // Sizing aplicado no handshake (recursos do fake backend).
    expect(port.sent).toContain('setoption name Threads value 4')
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

  it('dispose após análise concluída para o refino e encerra a engine', async () => {
    const port = fakeEnginePort()
    const { session } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })
    await session.start()

    session.dispose()
    await tick()

    expect(port.sent).toContain('stop')
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

describe('createReviewSession — refino ao vivo', () => {
  const tick = () => new Promise<void>((r) => setTimeout(r, 0))

  it('setDisplayedFen reponta a engine: stop + position fen + go infinite', async () => {
    const port = fakeEnginePort()
    const { session } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })
    await session.start()
    const len = port.sent.length

    session.setDisplayedFen(
      'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
    )
    await tick()

    expect(port.sent.slice(len)).toEqual([
      'stop',
      'position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
      'go infinite',
    ])
  })

  it('setDisplayedFen com o mesmo FEN é no-op (não reprepara a engine)', async () => {
    const port = fakeEnginePort()
    const { session } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
    })
    await session.start()
    const fen = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

    session.setDisplayedFen(fen)
    await tick()
    const afterFirst = port.sent.length

    session.setDisplayedFen(fen)
    await tick()

    expect(port.sent.length).toBe(afterFirst)
  })

  it('info da engine classifica o lance de variação pendente no store', async () => {
    const port = fakeEnginePort()
    const store = createReviewStore()
    const { session } = startSession({
      config: depthConfig(),
      backend: fakeBackend(port),
      store,
    })
    await session.start()

    // Usuário explora 1...c5 (variação no ply 1, lance pendente, focado).
    store.goTo(1)
    store.makeMove('c7c5')
    const v = store.getSnapshot().variations[1][0]
    expect(v.moves[0].classification).toBeUndefined()

    // A engine (fake) emite info para a posição em foco.
    port.emit('info depth 22 multipv 1 score cp -50 pv g1f3 d7d6')

    const vDepois = store.getSnapshot().variations[1][0]
    expect(vDepois.moves[0].afterCp).toBe(-50)
    expect(vDepois.moves[0].classification).toBeDefined()
    expect(vDepois.moves[0].bestUci).toBe('g1f3')
  })

  it('refino nasce apontando para o FEN exibido, não o final da partida', async () => {
    const port = fakeEnginePort()
    const backend = fakeBackend(port)
    let resolveSpawn!: (p: EnginePortHandle) => void
    backend.createEnginePort = () =>
      new Promise((resolve) => {
        resolveSpawn = resolve
      })
    const store = createReviewStore()
    const { session } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend,
      store,
    })
    const startPromise = session.start()
    await tick()

    // Reabertura instantânea: resultado já no store; usuário navega ao ply 0
    // enquanto a engine ainda não subiu.
    expect(store.getSnapshot().result).not.toBeNull()
    store.goTo(0)

    resolveSpawn(port)
    await startPromise

    const lastPos = port.sent.findLastIndex((c) => c.startsWith('position fen'))
    // posição exibida (ply 0), não a final (ply 2)
    expect(port.sent[lastPos]).toBe(
      'position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
    )
    expect(port.sent[lastPos + 1]).toBe('go infinite')
  })
})
