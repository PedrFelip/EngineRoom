import { vi } from 'vitest'
import type { ReviewConfig } from '../../../types'
import type { PositionCache } from '../../analyze'
import type { Backend, EnginePortHandle } from '../../backend'
import {
  createReviewSession,
  type ReviewSessionState,
} from '../../review-session'
import { createReviewStore } from '../../review-store'

/** Port que responde o handshake UCI e cada `go` com info + bestmove.
 * `emit` injeta uma linha UCI como se viesse do stdout da engine. */
export function fakeEnginePort(): EnginePortHandle & {
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

export function missCache(): PositionCache {
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

export function fakeBackend(
  port: EnginePortHandle,
): Backend & { saveReview: ReturnType<typeof vi.fn> } {
  return {
    createEnginePort: async () => port,
    getSystemResources: async () => ({ threads: 4, memory_mb: 8192 }),
    createPositionCache: () => missCache(),
    saveReview: vi.fn(async () => 1),
  }
}

export function depthConfig(
  overrides: Partial<ReviewConfig> = {},
): ReviewConfig {
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
export function existingResult() {
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

export function startSession(opts: {
  config: ReviewConfig
  backend: Backend
  store?: ReturnType<typeof createReviewStore>
}) {
  const states: ReviewSessionState[] = []
  const store = opts.store ?? createReviewStore()
  const onProgress = vi.fn()
  const session = createReviewSession({
    config: opts.config,
    backend: opts.backend,
    store,
    onStateChange: (s) => states.push(s),
    onProgress,
  })
  return { session, store, states, onProgress }
}
