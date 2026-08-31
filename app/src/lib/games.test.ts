import { describe, expect, it } from 'vitest'
import type { ReviewResult, StoredGame } from '../types'
import { storedToConfig } from './games'
import { ACCURACY_MODEL_VERSION } from './scoring'

const REVIEW: ReviewResult = {
  positions: [],
  moves: [],
  accuracyModel: ACCURACY_MODEL_VERSION,
  accuracy: { white: 98.5, black: 91 },
  accuracyByPhase: {
    opening: { white: 100, black: 100 },
    middlegame: { white: 100, black: 100 },
    endgame: { white: 100, black: 100 },
  },
}

function stored(overrides: Partial<StoredGame> = {}): StoredGame {
  return {
    id: 7,
    white: 'Brancas',
    black: 'Pretas',
    result: '1-0',
    plies: 2,
    engineTier: 'deep',
    mode: 'depth',
    depth: 25,
    multipv: 2,
    accuracyWhite: 98.5,
    accuracyBlack: 91,
    createdAt: '2026-07-17 20:00:00',
    pgn: '[White "Brancas"] [Black "Pretas"] [Result "1-0"] 1. e4 e5 1-0',
    reviewJson: JSON.stringify(REVIEW),
    ...overrides,
  }
}

describe('storedToConfig', () => {
  it('reabre partida salva como config com resultado pré-carregado', () => {
    const config = storedToConfig(stored())

    expect(config.engine.id).toBe('deep')
    expect(config.mode).toBe('depth')
    expect(config.lines).toBe(2)
    expect(config.initialResult).toEqual(REVIEW)
    expect(config.meta.white).toBe('Brancas')
    expect(config.meta.black).toBe('Pretas')
    expect(config.meta.result).toBe('1-0')
  })

  it('cai no tier pela depth quando o id é desconhecido', () => {
    const config = storedToConfig(stored({ engineTier: 'legado', depth: 15 }))

    expect(config.engine.id).toBe('fast')
  })

  it('reabre partida em modo time com movetimeMs lido do campo depth', () => {
    const config = storedToConfig(
      stored({ mode: 'time', engineTier: 'time', depth: 5000 }),
    )

    expect(config.mode).toBe('time')
    expect(config.movetimeMs).toBe(5000)
    expect(config.initialResult).toEqual(REVIEW)
  })

  it('reabre perfil automático sem tratar orçamento máximo como tempo manual', () => {
    const config = storedToConfig(
      stored({
        mode: 'time',
        analysisKind: 'auto-deep',
        engineTier: 'auto-deep',
        depth: 4000,
        multipv: 3,
      }),
    )

    expect(config.analysisKind).toBe('auto-deep')
    expect(config.movetimeMs).toBeUndefined()
    expect(config.lines).toBe(3)
  })

  it('normaliza revisão antiga (sem phase/accuracyByPhase) ao reabrir', () => {
    const START = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    const AFTER_E4 =
      'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
    const oldJson = JSON.stringify({
      positions: [
        { ply: 0, fen: START, depth: 20, cp: 0, winPct: 50, pv: [], lines: [] },
        {
          ply: 1,
          fen: AFTER_E4,
          depth: 20,
          cp: 0,
          winPct: 50,
          pv: [],
          lines: [],
        },
      ],
      moves: [],
      accuracy: { white: 100, black: 100 },
    })

    const config = storedToConfig(stored({ reviewJson: oldJson }))
    const result = config.initialResult

    // material cheio → todas as posições viram Abertura; accuracyByPhase preenchido
    expect(result.positions.every((p) => p.phase === 'opening')).toBe(true)
    expect(result.accuracyByPhase.opening).toEqual({ white: 100, black: 100 })
  })

  it('deriva cpLoss e recalcula accuracy de uma revisão antiga', () => {
    const START = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    const AFTER_F3 =
      'rnbqkbnr/pppppppp/8/8/8/5P2/PPPPP1PP/RNBQKBNR b KQkq - 0 1'
    const legacy = {
      positions: [
        {
          ply: 0,
          fen: START,
          phase: 'opening',
          depth: 20,
          cp: 0,
          winPct: 50,
          pv: ['e2e4'],
          lines: [],
        },
        {
          ply: 1,
          fen: AFTER_F3,
          phase: 'opening',
          depth: 20,
          cp: 500,
          winPct: 13.7,
          pv: ['d8h4'],
          lines: [],
        },
      ],
      moves: [
        {
          ply: 1,
          color: 'w',
          san: 'f3',
          uci: 'f2f3',
          fenBefore: START,
          classification: 'blunder',
          winPctBefore: 50,
          winPctAfter: 13.7,
          winPctLoss: 36.3,
          bestUci: 'e2e4',
          isBook: false,
          eco: null,
        },
      ],
      accuracy: { white: 15, black: 100 },
      accuracyByPhase: {
        opening: { white: 15, black: 100 },
        middlegame: { white: 100, black: 100 },
        endgame: { white: 100, black: 100 },
      },
    }

    const config = storedToConfig(
      stored({ reviewJson: JSON.stringify(legacy) }),
    )
    const result = config.initialResult

    expect(result.moves[0].cpLoss).toBe(500)
    expect(result.accuracy.white).toBeCloseTo(17.8, 1)
    expect(result.accuracyByPhase.opening.white).toBeCloseTo(17.8, 1)
  })
})
