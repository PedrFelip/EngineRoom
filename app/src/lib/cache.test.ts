import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))

import { createTauriPositionCache } from './cache'

beforeEach(() => {
  mocks.invoke.mockReset()
})

describe('createTauriPositionCache (infinite mode)', () => {
  it('putInfinite stores under mode=infinite with sentinel depth=0 and the depth in the JSON payload', async () => {
    mocks.invoke.mockResolvedValue(undefined)
    const cache = createTauriPositionCache()
    const putInfinite = cache.putInfinite
    if (!putInfinite) throw new Error('putInfinite deve estar definido')

    await putInfinite({
      fen: 'rnbqkbnr/8/8/8/8/8/8/8/RNBQKBNR w KQkq - 0 1',
      cp: 42,
      depth: 30,
      pv: ['e2e4'],
      lines: [
        { multipv: 1, cp: 42, pv: ['e2e4'] },
        { multipv: 2, cp: 10, pv: ['d2d4'] },
      ],
    })

    expect(mocks.invoke).toHaveBeenCalledWith('cache_put', {
      fen: 'rnbqkbnr/8/8/8/8/8/8/8/RNBQKBNR w KQkq - 0 1',
      mode: 'infinite',
      depth: 0,
      multipv: 1,
      cp: 42,
      linesJson: JSON.stringify({
        depth: 30,
        lines: [
          { multipv: 1, cp: 42, pv: ['e2e4'] },
          { multipv: 2, cp: 10, pv: ['d2d4'] },
        ],
      }),
    })
  })

  it('getInfinite round-trips depth and lines', async () => {
    mocks.invoke.mockResolvedValue({
      cp: 42,
      linesJson: JSON.stringify({
        depth: 30,
        lines: [{ multipv: 1, cp: 42, pv: ['e2e4'] }],
      }),
    })
    const cache = createTauriPositionCache()
    const getInfinite = cache.getInfinite
    if (!getInfinite) throw new Error('getInfinite deve estar definido')

    const hit = await getInfinite(
      'rnbqkbnr/8/8/8/8/8/8/8/RNBQKBNR w KQkq - 0 1',
    )
    expect(hit).not.toBeNull()
    expect(hit?.cp).toBe(42)
    expect(hit?.depth).toBe(30)
    expect(hit?.pv).toEqual(['e2e4'])
    expect(hit?.lines).toEqual([{ multipv: 1, cp: 42, pv: ['e2e4'] }])

    expect(mocks.invoke).toHaveBeenCalledWith('cache_get', {
      fen: 'rnbqkbnr/8/8/8/8/8/8/8/RNBQKBNR w KQkq - 0 1',
      mode: 'infinite',
      depth: 0,
      multipv: 1,
    })
  })

  it('getInfinite returns null when no row is cached', async () => {
    mocks.invoke.mockResolvedValue(null)
    const cache = createTauriPositionCache()
    const getInfinite = cache.getInfinite
    if (!getInfinite) throw new Error('getInfinite deve estar definido')
    const hit = await getInfinite('some-fen')
    expect(hit).toBeNull()
  })
})
