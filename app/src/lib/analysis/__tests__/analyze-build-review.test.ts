import { describe, expect, it } from 'vitest'
import { buildReview } from '../../analyze'

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
      { fen: AFTER_E4, cp: -15, depth: 20, pv: ['e7e5'] },
      { fen: AFTER_E5, cp: 15, depth: 20, pv: [] },
    ]

    const review = buildReview(game, raw)

    expect(review.positions).toHaveLength(3)
    expect(review.moves).toHaveLength(2)
    expect(review.moves[0].classification).toBe('melhor')
    expect(review.moves[1].classification).toBe('melhor')
    expect(review.moves[0].winPctLoss).toBe(0)
    expect(review.moves[0].cpLoss).toBe(0)
    expect(review.moves[1].cpLoss).toBe(0)
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
    expect(review.moves[0].cpLoss).toBe(500)
    expect(review.moves[0].bestUci).toBe('e2e4')
    expect(review.accuracy.white).toBeCloseTo(17.8, 1)
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
    expect(review.moves[1].cpLoss).toBe(500)
    expect(review.accuracy.black).toBeCloseTo(19.1, 1)
    expect(review.accuracy.white).toBeCloseTo(95, 0)
  })

  it('marca lances de abertura como Livro e os inclui na precisão', () => {
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
      { fen: AFTER_E4, cp: 100, depth: 20, pv: ['e7e5'] },
      { fen: AFTER_E5, cp: -100, depth: 20, pv: ['g1f3'] },
    ]
    const book = {
      maxPly: 1,
      eco: { code: 'B00', name: "King's Pawn", moves: ['e4'] },
    }

    const review = buildReview(game, raw, book)

    expect(review.moves[0].classification).toBe('livro')
    expect(review.moves[0].isBook).toBe(true)
    expect(review.moves[0].eco).toEqual({ code: 'B00', name: "King's Pawn" })
    expect(review.moves[1].classification).toBe('melhor')
    expect(review.moves[1].isBook).toBe(false)
    expect(review.moves[1].eco).toBeNull()
    expect(review.accuracy.white).toBeLessThan(100)
    expect(review.accuracy.black).toBe(100)
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
    expect(review.moves[0].cpLoss).toBe(0)
    expect(review.moves[0].classification).toBe('melhor')
    expect(review.positions[1].winPct).toBeCloseTo(97.5, 1)
  })

  it('preenche phase das posições e accuracyByPhase por fase', () => {
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
      { fen: AFTER_E4, cp: -15, depth: 20, pv: ['e7e5'] },
      { fen: AFTER_E5, cp: 15, depth: 20, pv: [] },
    ]

    const review = buildReview(game, raw)

    // material cheio (e4/e5 não capturam) → todas as posições são Abertura
    expect(review.positions.every((p) => p.phase === 'opening')).toBe(true)
    // acurácia da Abertura espelha a acurácia geral; demais fases vazias → 100
    expect(review.accuracyByPhase.opening).toEqual({
      white: 100,
      black: 100,
    })
    expect(review.accuracyByPhase.middlegame).toEqual({
      white: 100,
      black: 100,
    })
    expect(review.accuracyByPhase.endgame).toEqual({ white: 100, black: 100 })
  })
})
