import { describe, expect, it } from 'vitest'
import { buildReview } from '../../analyze'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const AFTER_E4 = 'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
const _AFTER_E5 =
  'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2'

describe('buildReview — alternativas MultiPV', () => {
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
    ],
  }

  it('não promove o único lance que mantém a igualdade', () => {
    const raw = [
      {
        fen: START_FEN,
        cp: 0,
        depth: 20,
        pv: ['e2e4'],
        lines: [
          { multipv: 1, cp: 0, pv: ['e2e4'] },
          { multipv: 2, cp: -200, pv: ['d2d4'] },
        ],
      },
      { fen: AFTER_E4, cp: 0, depth: 20, pv: ['e7e5'] },
    ]

    expect(buildReview(game, raw).moves[0].classification).toBe('melhor')
  })

  it('não promove sem uma segunda linha que atravesse a faixa de resultado', () => {
    const raw = [
      {
        fen: START_FEN,
        cp: 0,
        depth: 20,
        pv: ['e2e4'],
        lines: [
          { multipv: 1, cp: 0, pv: ['e2e4'] },
          { multipv: 2, cp: -100, pv: ['d2d4'] },
        ],
      },
      { fen: AFTER_E4, cp: 0, depth: 20, pv: ['e7e5'] },
    ]

    expect(buildReview(game, raw).moves[0].classification).toBe('melhor')
  })

  it('mantém Livro acima da classificação por perda', () => {
    const raw = [
      {
        fen: START_FEN,
        cp: 0,
        depth: 20,
        pv: ['e2e4'],
        lines: [
          { multipv: 1, cp: 0, pv: ['e2e4'] },
          { multipv: 2, cp: -200, pv: ['d2d4'] },
        ],
      },
      { fen: AFTER_E4, cp: 0, depth: 20, pv: ['e7e5'] },
    ]
    const book = {
      maxPly: 1,
      eco: { code: 'B00', name: 'Abertura', moves: ['e4'] },
    }

    expect(buildReview(game, raw, book).moves[0].classification).toBe('livro')
  })
})
