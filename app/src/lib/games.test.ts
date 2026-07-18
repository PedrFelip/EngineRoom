import { describe, expect, it } from 'vitest'
import type { ReviewResult, StoredGame } from '../types'
import { storedToConfig } from './games'

const REVIEW: ReviewResult = {
  positions: [],
  moves: [],
  accuracy: { white: 98.5, black: 91 },
}

function stored(overrides: Partial<StoredGame> = {}): StoredGame {
  return {
    id: 7,
    white: 'Brancas',
    black: 'Pretas',
    result: '1-0',
    plies: 2,
    engineTier: 'deep',
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
})
