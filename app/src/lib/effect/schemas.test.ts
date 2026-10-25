import { Schema } from 'effect'
import { describe, expect, it } from 'vitest'
import {
  CachedPositionSchema,
  EngineExitSchema,
  ResourcesSchema,
  StoredGameSchema,
} from './schemas'

// Compatibility at the IPC/history boundary must survive Schema major upgrades.
describe('IPC schema compatibility', () => {
  it('defaults missing cache depth but rejects invalid explicit values', () => {
    const decode = Schema.decodeUnknownSync(CachedPositionSchema)
    expect(decode({ cp: 0, linesJson: '[]' }).reachedDepth).toBe(0)
    expect(
      decode({ cp: 0, linesJson: '[]', reachedDepth: undefined }).reachedDepth,
    ).toBe(0)
    for (const reachedDepth of [
      null,
      -1,
      1.5,
      Number.NaN,
      Number.POSITIVE_INFINITY,
    ]) {
      expect(() => decode({ cp: 0, linesJson: '[]', reachedDepth })).toThrow()
    }
  })

  it('reopens old history without a mode and preserves explicit time budgets', () => {
    const decode = Schema.decodeUnknownSync(StoredGameSchema)
    const old = {
      id: 1,
      white: 'White',
      black: 'Black',
      result: '*',
      plies: 2,
      engineTier: 'balanced',
      depth: 20,
      multipv: 3,
      accuracyWhite: 90,
      accuracyBlack: 80,
      createdAt: '2026-10-25',
      pgn: '1. e4 e5',
      reviewJson: '{}',
    }
    expect(decode(old).mode).toBe('depth')
    expect(decode({ ...old, mode: 'time' }).mode).toBe('time')
    expect(() => decode({ ...old, mode: null })).toThrow()
  })

  it('accepts absent and null process errors while preserving actual errors', () => {
    const decode = Schema.decodeUnknownSync(EngineExitSchema)
    for (const error of [undefined, null]) {
      expect(decode({ code: null, signal: null, error }).error).toBeUndefined()
    }
    expect(decode({ code: 1, signal: null, error: 'exited' }).error).toBe(
      'exited',
    )
    expect(() => decode({ code: null, signal: null, error: 1 })).toThrow()
  })

  it('keeps resource counts finite, integral and strictly positive', () => {
    const decode = Schema.decodeUnknownSync(ResourcesSchema)
    expect(decode({ threads: 1, memory_mb: 16 })).toEqual({
      threads: 1,
      memory_mb: 16,
    })
    for (const threads of [0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => decode({ threads, memory_mb: 16 })).toThrow()
    }
  })
})
