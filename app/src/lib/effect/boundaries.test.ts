import { Effect } from 'effect'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))

import type { StoredGame } from '../../types'
import {
  createTauriPositionCache,
  shapeCachedPosition,
} from '../__tests__/reference/cache'
import { runUi } from '../__tests__/reference/effect/ui-runtime'
import { listGames, storedToConfig } from '../__tests__/reference/games'
import { getStorageStats } from '../storage'
import { getSystemResources } from '../system'
import { decode } from './ipc'
import { EngineExitSchema, LegacyReviewSchema } from './schemas'

beforeEach(() => {
  mocks.invoke.mockReset()
})

describe('validated boundaries', () => {
  it.each([
    null,
    {},
    { cp: '0', linesJson: '[]' },
    { cp: 0, linesJson: '{' },
    { cp: 0, linesJson: '[{"multipv":1,"cp":0,"pv":42}]' },
    { cp: Infinity, linesJson: '[]' },
  ])('treats a corrupt cache row as a miss: %j', (row) => {
    expect(shapeCachedPosition(row, 'fen', 1)).toBeNull()
  })

  it('keeps valid bulk hits in their original positions, including legacy depths', async () => {
    mocks.invoke.mockResolvedValue([
      {
        cp: 42,
        linesJson: '[{"multipv":1,"cp":42,"pv":["e2e4"]}]',
        reachedDepth: 20,
      },
      { cp: 10, linesJson: 'broken', reachedDepth: 30 },
      null,
    ])
    const hits = await runUi(
      createTauriPositionCache().getBulk(
        ['one', 'two', 'three'],
        'time',
        5000,
        1,
      ),
    )
    expect(hits[0]).toMatchObject({ fen: 'one', cp: 42, depth: 20 })
    expect(hits.slice(1)).toEqual([null, null])
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith('cache_get_bulk', {
      fens: ['one', 'two', 'three'],
      mode: 'time',
      depth: 5000,
      multipv: 1,
    })
  })

  it('does not shift bulk hits when the response has the wrong length', async () => {
    mocks.invoke.mockResolvedValue([])
    expect(
      await runUi(
        createTauriPositionCache().getBulk(['a', 'b'], 'depth', 20, 1),
      ),
    ).toEqual([null, null])
  })

  it('propagates cache I/O failures rather than treating them as misses', async () => {
    mocks.invoke.mockRejectedValue(new Error('disk error'))
    await expect(
      runUi(createTauriPositionCache().get('fen', 'time', 20, 1)),
    ).rejects.toMatchObject({
      _tag: 'CacheError',
      operation: 'cache_get',
      message: 'disk error',
    })
  })

  it('rejects malformed resources, storage stats and history payloads', async () => {
    mocks.invoke.mockResolvedValue({ threads: 0, memory_mb: -1 })
    await expect(runUi(getSystemResources())).rejects.toMatchObject({
      _tag: 'InvalidPayloadError',
    })
    mocks.invoke.mockResolvedValue({
      cacheBytes: -1,
      gamesBytes: 0,
      dbBytes: 0,
    })
    await expect(runUi(getStorageStats())).rejects.toMatchObject({
      _tag: 'InvalidPayloadError',
    })
    mocks.invoke.mockResolvedValue({ games: [], total: '0', nextCursor: null })
    await expect(runUi(listGames(50))).rejects.toMatchObject({
      _tag: 'InvalidPayloadError',
    })
  })

  it('fails malformed review JSON before attempting normalization', async () => {
    const game = { reviewJson: '{"positions":42}' } as StoredGame
    await expect(runUi(storedToConfig(game))).rejects.toMatchObject({
      _tag: 'InvalidPayloadError',
      source: 'revisão salva',
    })
  })

  it('accepts optional legacy fields but rejects invalid nested values', async () => {
    const old = JSON.stringify({
      positions: [],
      moves: [],
      accuracy: { white: 100, black: 100 },
    })
    await expect(
      runUi(decode(LegacyReviewSchema, old, 'legacy')),
    ).resolves.toMatchObject({ positions: [] })
    await expect(
      runUi(decode(EngineExitSchema, { code: 0, signal: '11' }, 'exit')),
    ).rejects.toMatchObject({ _tag: 'InvalidPayloadError' })
    const failed = await Effect.runPromise(
      Effect.flip(decode(LegacyReviewSchema, '{}', 'review')),
    )
    expect(failed.source).toBe('review')
  })

  it('accepts the null error serialized by Rust Option<String>', async () => {
    const decoded = await runUi(
      decode(EngineExitSchema, { code: null, signal: 11, error: null }, 'exit'),
    )
    expect(decoded).toEqual({ code: null, signal: 11 })
  })
})
