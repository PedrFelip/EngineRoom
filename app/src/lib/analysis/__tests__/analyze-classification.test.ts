import { describe, expect, it } from 'vitest'
import { buildReview } from '../../analyze'

const _START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const _AFTER_E4 = 'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
const _AFTER_E5 =
  'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2'

describe('buildReview — classificação objetiva', () => {
  // Bispo em c4 captura o peão de f7; rei recaptura: -2 peões (sacrifício).
  const SAC_BEFORE =
    'rnbqkbnr/pppppppp/8/8/2B1P3/8/PPPP1PPP/RNBQK1NR w KQkq - 0 1'
  const SAC_AFTER = 'rnbqkbnr/ppppBppp/8/8/4P3/8/PPPP1PPP/RNBQK1NR b KQkq - 0 1'
  // exd5 Qxd5: troca simétrica de peões, delta material zero.
  const TRADE_BEFORE =
    'rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3'
  const TRADE_AFTER =
    'rnbqkbnr/ppp1pppp/8/3P4/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 3'
  const sacGame = {
    startFen: SAC_BEFORE,
    moves: [
      {
        ply: 1,
        color: 'w' as const,
        san: 'Bxf7+',
        uci: 'c4f7',
        fenBefore: SAC_BEFORE,
      },
    ],
  }

  it('classifica sacrifício sem perda de win% como Melhor', () => {
    const raw = [
      { fen: SAC_BEFORE, cp: 30, depth: 20, pv: ['c4f7'] },
      { fen: SAC_AFTER, cp: -30, depth: 20, pv: ['e8f7'] },
    ]

    const review = buildReview(sacGame, raw)

    expect(review.moves[0].classification).toBe('melhor')
    expect(review.moves[0].winPctLoss).toBe(0)
  })

  it('classifica troca simétrica sem perda como Melhor', () => {
    const game = {
      startFen: TRADE_BEFORE,
      moves: [
        {
          ply: 1,
          color: 'w' as const,
          san: 'exd5',
          uci: 'e4d5',
          fenBefore: TRADE_BEFORE,
        },
      ],
    }
    const raw = [
      { fen: TRADE_BEFORE, cp: 30, depth: 20, pv: ['e4d5'] },
      { fen: TRADE_AFTER, cp: -30, depth: 20, pv: ['d8d5'] },
    ]

    expect(buildReview(game, raw).moves[0].classification).toBe('melhor')
  })

  it('classifica lance sem perda como Melhor mesmo em posição ganha', () => {
    const raw = [
      { fen: SAC_BEFORE, cp: 600, depth: 20, pv: ['c4f7'] },
      { fen: SAC_AFTER, cp: -600, depth: 20, pv: ['e8f7'] },
    ]

    expect(buildReview(sacGame, raw).moves[0].classification).toBe('melhor')
  })

  it('classifica pela perda quando o lance deixa a posição ruim', () => {
    const raw = [
      { fen: SAC_BEFORE, cp: 30, depth: 20, pv: ['c4f7'] },
      { fen: SAC_AFTER, cp: 300, depth: 20, pv: ['e8f7'] },
    ]

    // cp +300 no POV das pretas ≈ win% 17 para as brancas → Blunder.
    expect(buildReview(sacGame, raw).moves[0].classification).toBe('blunder')
  })

  it('MultiPV não promove a classificação baseada em win%', () => {
    const rawSingle = [
      { fen: SAC_BEFORE, cp: 30, depth: 20, pv: ['c4f7'] },
      { fen: SAC_AFTER, cp: -28, depth: 20, pv: ['e8f7'] },
    ]
    // Perda ~0,2 win% → Excelente, independentemente da 2ª linha.
    expect(buildReview(sacGame, rawSingle).moves[0].classification).toBe(
      'excelente',
    )

    const rawMulti = [
      {
        fen: SAC_BEFORE,
        cp: 30,
        depth: 20,
        pv: ['c4f7'],
        lines: [
          { multipv: 1, cp: 30, pv: ['c4f7'] },
          { multipv: 2, cp: 25, pv: ['d2d4'] },
        ],
      },
      { fen: SAC_AFTER, cp: -28, depth: 20, pv: ['e8f7'] },
    ]
    expect(buildReview(sacGame, rawMulti).moves[0].classification).toBe(
      'excelente',
    )
  })

  it('mantém Livro acima da classificação por perda', () => {
    const raw = [
      { fen: SAC_BEFORE, cp: 30, depth: 20, pv: ['c4f7'] },
      { fen: SAC_AFTER, cp: -30, depth: 20, pv: ['e8f7'] },
    ]
    const book = {
      maxPly: 1,
      eco: { code: 'C00', name: 'Gambito', moves: ['Bxf7+'] },
    }

    expect(buildReview(sacGame, raw, book).moves[0].classification).toBe(
      'livro',
    )
  })
})
