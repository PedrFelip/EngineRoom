import { describe, expect, it } from 'vitest'
import {
  analyzeGame,
  buildReview,
  type EnginePort,
  type PositionCache,
} from './analyze'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const AFTER_E4 = 'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
const AFTER_E5 = 'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2'

describe('buildReview', () => {
  it('classifica como Melhor e dá precisão 100 numa partida com avaliações iguais', () => {
    const game = {
      startFen: START_FEN,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'e4',
          uci: 'e2e4',
          fenBefore: START_FEN,
        },
        {
          ply: 2,
          color: 'b' as const,
          san: 'e5',
          uci: 'e7e5',
          fenBefore: AFTER_E4,
        },
      ],
    }
    const raw = [
      { fen: START_FEN, cp: 0, depth: 20, pv: ['e2e4', 'e7e5'] },
      { fen: AFTER_E4, cp: 0, depth: 20, pv: ['e7e5'] },
      { fen: AFTER_E5, cp: 0, depth: 20, pv: [] },
    ]

    const review = buildReview(game, raw)

    expect(review.positions).toHaveLength(3)
    expect(review.moves).toHaveLength(2)
    expect(review.moves[0].classification).toBe('melhor')
    expect(review.moves[1].classification).toBe('melhor')
    expect(review.moves[0].winPctLoss).toBe(0)
    expect(review.accuracy).toEqual({ white: 100, black: 100 })
  })

  it('classifica como Blunder quando um lance entrega vantagem', () => {
    const game = {
      startFen: START_FEN,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'f3',
          uci: 'f2f3',
          fenBefore: START_FEN,
        },
      ],
    }
    const raw = [
      { fen: START_FEN, cp: 0, depth: 20, pv: ['e2e4'] },
      { fen: AFTER_E4, cp: 500, depth: 20, pv: ['d8h4'] },
    ]

    const review = buildReview(game, raw)

    expect(review.moves[0].classification).toBe('blunder')
    expect(review.moves[0].winPctAfter).toBeCloseTo(13.7, 1)
    expect(review.moves[0].winPctLoss).toBeCloseTo(36.3, 1)
    expect(review.moves[0].bestUci).toBe('e2e4')
    expect(review.accuracy.white).toBeLessThan(30)
    expect(review.accuracy.black).toBe(100)
  })

  it('armazena win% das posições no ponto de vista das brancas', () => {
    const game = {
      startFen: START_FEN,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'f3',
          uci: 'f2f3',
          fenBefore: START_FEN,
        },
      ],
    }
    const raw = [
      { fen: START_FEN, cp: 0, depth: 20, pv: ['e2e4'] },
      { fen: AFTER_E4, cp: 500, depth: 20, pv: ['d8h4'] },
    ]

    const review = buildReview(game, raw)

    expect(review.positions[0].winPct).toBeCloseTo(50, 0)
    expect(review.positions[1].winPct).toBeCloseTo(13.7, 1)
  })

  it('trata o sinal corretamente para lances das pretas', () => {
    const game = {
      startFen: START_FEN,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'e4',
          uci: 'e2e4',
          fenBefore: START_FEN,
        },
        {
          ply: 2,
          color: 'b' as const,
          san: 'f5',
          uci: 'f7f5',
          fenBefore: AFTER_E4,
        },
      ],
    }
    const raw = [
      { fen: START_FEN, cp: 0, depth: 20, pv: ['e2e4'] },
      { fen: AFTER_E4, cp: 0, depth: 20, pv: ['e7e5'] },
      { fen: AFTER_E5, cp: 500, depth: 20, pv: ['e4f5'] },
    ]

    const review = buildReview(game, raw)

    expect(review.moves[0].classification).toBe('melhor')
    expect(review.moves[1].classification).toBe('blunder')
    expect(review.moves[1].winPctLoss).toBeCloseTo(36.3, 1)
    expect(review.accuracy.black).toBeLessThan(30)
    expect(review.accuracy.white).toBe(100)
  })

  it('marca lances de abertura como Livro e exclui da precisão', () => {
    const game = {
      startFen: START_FEN,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'e4',
          uci: 'e2e4',
          fenBefore: START_FEN,
        },
        {
          ply: 2,
          color: 'b' as const,
          san: 'e5',
          uci: 'e7e5',
          fenBefore: AFTER_E4,
        },
      ],
    }
    const raw = [
      { fen: START_FEN, cp: 0, depth: 20, pv: ['e2e4'] },
      { fen: AFTER_E4, cp: 0, depth: 20, pv: ['e7e5'] },
      { fen: AFTER_E5, cp: 500, depth: 20, pv: ['e4e5'] },
    ]
    const book = {
      maxPly: 1,
      eco: { code: 'B00', name: "King's Pawn", moves: ['e4'] },
    }

    const review = buildReview(game, raw, book)

    expect(review.moves[0].classification).toBe('livro')
    expect(review.moves[0].isBook).toBe(true)
    expect(review.moves[0].eco).toEqual({ code: 'B00', name: "King's Pawn" })
    expect(review.moves[1].classification).toBe('blunder')
    expect(review.moves[1].isBook).toBe(false)
    expect(review.moves[1].eco).toBeNull()
    expect(review.accuracy.black).toBeLessThan(30)
  })

  it('não pune o lance que dá xeque-mate (fora do livro)', () => {
    const game = {
      startFen: START_FEN,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'Qh5#',
          uci: 'd1h5',
          fenBefore: START_FEN,
        },
      ],
    }
    const raw = [
      { fen: START_FEN, cp: 0, depth: 20, pv: ['d1h5'] },
      {
        fen: '8k',
        cp: -100000,
        depth: 0,
        pv: [],
        lines: [{ multipv: 1, cp: -100000, pv: [] }],
      },
    ]

    const review = buildReview(game, raw)

    expect(review.moves[0].winPctLoss).toBe(0)
    expect(review.moves[0].classification).toBe('melhor')
    expect(review.positions[1].winPct).toBeCloseTo(100, 0)
  })
})

function fakePort(
  evalFor: (fen: string) => { cp: number; pv: string[]; depth?: number },
): EnginePort {
  let lineCb: ((line: string) => void) | null = null
  let currentFen = ''
  return {
    send(cmd: string) {
      const c = cmd.trim()
      if (c === 'uci') lineCb?.('uciok')
      else if (c === 'isready') lineCb?.('readyok')
      else if (c.startsWith('position fen'))
        currentFen = c.slice('position fen'.length).trim()
      else if (c.startsWith('go')) {
        const { cp, pv, depth = 20 } = evalFor(currentFen)
        lineCb?.(
          `info depth ${depth} multipv 1 score cp ${cp} pv ${pv.join(' ')}`,
        )
        lineCb?.(`bestmove ${pv[0] ?? 'e2e4'}`)
      }
    },
    onLine(handler: (line: string) => void) {
      lineCb = handler
      return () => {
        lineCb = null
      }
    },
  }
}

describe('analyzeGame', () => {
  it('aciona o engine por ply e devolve a revisão', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const review = await analyzeGame('1. e4 e5', 20, port)

    expect(review.positions).toHaveLength(3)
    expect(review.moves).toHaveLength(2)
    expect(review.positions.every((p) => Math.abs(p.winPct - 50) < 0.1)).toBe(
      true,
    )
    expect(review.moves.every((m) => m.winPctLoss === 0)).toBe(true)
    expect(review.accuracy).toEqual({ white: 100, black: 100 })
  })

  it('propaga a avaliação por FEN até a perda de win% do lance', async () => {
    const port = fakePort((fen) =>
      fen === START_FEN ? { cp: 0, pv: ['e2e4'] } : { cp: 500, pv: ['d8h4'] },
    )
    const review = await analyzeGame('1. e4', 20, port)

    expect(review.moves).toHaveLength(1)
    expect(review.moves[0].winPctLoss).toBeCloseTo(36.3, 1)
    expect(review.moves[0].bestUci).toBe('e2e4')
    expect(review.moves[0].classification).toBe('livro')
  })

  it('coleta múltiplas linhas candidatas quando multipv > 1', async () => {
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd: string) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          lineCb?.('info depth 20 multipv 1 score cp 30 pv e2e4 e7e5')
          lineCb?.('info depth 20 multipv 2 score cp 10 pv d2d4 d7d5')
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler: (line: string) => void) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }

    const review = await analyzeGame('1. e4 e5', 20, port, 2)
    const lines0 = review.positions[0].lines

    expect(lines0).toHaveLength(2)
    expect(lines0[0].multipv).toBe(1)
    expect(lines0[0].san).toBe('e4')
    expect(lines0[1].multipv).toBe(2)
    expect(lines0[1].san).toBe('d4')
    expect(lines0[0].winPct).toBeGreaterThan(lines0[1].winPct)
  })

  it('resolve xeque-mate deterministicamente (sem depender da engine)', async () => {
    const port = fakePort(() => ({ cp: 0, pv: [] }))
    const review = await analyzeGame('1. f3 e5 2. g4 Qh4#', 20, port)

    const last = review.positions[review.positions.length - 1]
    expect(last.winPct).toBeCloseTo(0, 0)

    const mateMove = review.moves[review.moves.length - 1]
    expect(mateMove.winPctLoss).toBe(0)
  })

  it('usa o cache de posições em vez de acionar o engine', async () => {
    let gos = 0
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd: string) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          gos++
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler: (line: string) => void) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }
    const cache: PositionCache = {
      async get(fen) {
        return {
          fen,
          cp: 0,
          depth: 20,
          pv: ['e2e4'],
          lines: [{ multipv: 1, cp: 0, pv: ['e2e4'], san: 'e4' }],
        }
      },
      async put() {
        throw new Error('cache cheio não deveria gravar')
      },
    }

    const review = await analyzeGame('1. e4 e5', 20, port, 1, { cache })

    expect(gos).toBe(0)
    expect(review.moves).toHaveLength(2)
    expect(review.moves.every((m) => m.winPctLoss === 0)).toBe(true)
  })

  it('avalia no engine e grava no cache a posição ainda não cacheada', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const gravadas: { fen: string; depth: number; multipv: number }[] = []
    const cache: PositionCache = {
      async get() {
        return null
      },
      async put(pos, depth, multipv) {
        gravadas.push({ fen: pos.fen, depth, multipv })
      },
    }

    const review = await analyzeGame('1. e4 e5', 18, port, 2, { cache })

    expect(gravadas).toHaveLength(3)
    expect(gravadas.map((g) => g.fen)).toEqual([
      START_FEN,
      'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1',
      'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNB w KQkq - 0 2',
    ])
    expect(gravadas.every((g) => g.depth === 18 && g.multipv === 2)).toBe(true)
    expect(review.moves).toHaveLength(2)
  })

  it('em modo tempo envia `go movetime N` para a engine (nunca `go depth`)', async () => {
    const sent: string[] = []
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        sent.push(cmd.trim())
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          lineCb?.('info depth 28 multipv 1 score cp 0 pv e2e4 e7e5')
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }

    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'time', movetimeMs: 1000 },
      port,
    )

    expect(sent).toContain('go movetime 1000')
    expect(sent.some((c) => c.startsWith('go depth'))).toBe(false)
    expect(review.moves).toHaveLength(2)
    expect(review.accuracy).toEqual({ white: 100, black: 100 })
  })
})
