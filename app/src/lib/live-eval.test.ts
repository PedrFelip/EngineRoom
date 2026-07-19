import { describe, expect, it, vi } from 'vitest'
import type { EnginePort, PositionCache, RawPosition } from './analyze'
import { createLiveEvalSession } from './live-eval'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

function fakePort() {
  const handlers = new Set<(line: string) => void>()
  const sent: string[] = []
  return {
    sent,
    port: {
      send: (line: string) => {
        sent.push(line)
      },
      onLine: (h: (line: string) => void) => {
        handlers.add(h)
        return () => {
          handlers.delete(h)
        }
      },
    } satisfies EnginePort,
    emit(line: string) {
      handlers.forEach((h) => {
        h(line)
      })
    },
  }
}

describe('createLiveEvalSession', () => {
  it('forwards each info line as a RawPosition via onMerge', async () => {
    const { port, emit } = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: port, wide: null },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )

    await session.start()

    emit('info depth 20 multipv 1 score cp 30 pv e2e4 e7e5')

    expect(merged).toHaveLength(1)
    expect(merged[0]).toMatchObject({
      fen: START_FEN,
      cp: 30,
      depth: 20,
      pv: ['e2e4', 'e7e5'],
    })
  })

  it('keeps the deepest info for a slot, ignoring shallower follow-ups', async () => {
    const { port, emit } = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: port, wide: null },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()

    emit('info depth 20 multipv 1 score cp 30 pv e2e4')
    emit('info depth 15 multipv 1 score cp 50 pv d2d4') // shallower → ignored
    emit('info depth 25 multipv 1 score cp 40 pv c2c4') // deeper → replaces

    expect(merged).toHaveLength(2)
    expect(merged[0].depth).toBe(20)
    expect(merged[merged.length - 1]).toMatchObject({
      depth: 25,
      cp: 40,
      pv: ['c2c4'],
    })
  })

  it('tracks each multipv slot independently and lists them in order', async () => {
    const { port, emit } = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: port, wide: null },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()

    emit('info depth 18 multipv 1 score cp 25 pv e2e4')
    emit('info depth 16 multipv 2 score cp 10 pv d2d4')
    emit('info depth 14 multipv 3 score cp -5 pv c2c4')

    const last = merged[merged.length - 1]
    expect(last.lines).toEqual([
      { multipv: 1, cp: 25, pv: ['e2e4'] },
      { multipv: 2, cp: 10, pv: ['d2d4'] },
      { multipv: 3, cp: -5, pv: ['c2c4'] },
    ])
  })

  it('merges deep and wide engines, picking the deepest per slot', async () => {
    // Deep reached slot 1 at depth 30. Wide reaches slot 1 at depth 20 (shallower)
    // plus slots 2 and 3 that deep never saw. Merge: slot 1 from deep, slots 2-3
    // from wide.
    const deep = fakePort()
    const wide = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: deep.port, wide: wide.port },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()

    deep.emit('info depth 30 multipv 1 score cp 42 pv e2e4')
    wide.emit('info depth 20 multipv 1 score cp 40 pv e2e4')
    wide.emit('info depth 18 multipv 2 score cp 15 pv d2d4')
    wide.emit('info depth 16 multipv 3 score cp -8 pv c2c4')

    const last = merged[merged.length - 1]
    expect(last.lines).toEqual([
      { multipv: 1, cp: 42, pv: ['e2e4'] }, // from deep (deeper)
      { multipv: 2, cp: 15, pv: ['d2d4'] }, // from wide
      { multipv: 3, cp: -8, pv: ['c2c4'] }, // from wide
    ])
    expect(last.cp).toBe(42)
    expect(last.depth).toBe(30)
    expect(last.pv).toEqual(['e2e4'])
  })

  it('setFen stops, clears state, and restarts on the new fen', async () => {
    const deep = fakePort()
    const wide = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: deep.port, wide: wide.port },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()
    deep.sent.length = 0
    wide.sent.length = 0

    // Build some state on the old fen.
    deep.emit('info depth 10 multipv 1 score cp 5 pv e2e4')
    const beforeSwitch = merged.length

    const nextFen =
      'r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1'
    await session.setFen(nextFen)

    // Both engines got stop + position + go.
    expect(deep.sent).toEqual([
      'stop',
      `position fen ${nextFen}`,
      'go infinite',
    ])
    expect(wide.sent).toEqual([
      'stop',
      `position fen ${nextFen}`,
      'go infinite',
    ])

    // State was cleared: a shallower info now produces a fresh merge (otherwise
    // it would have been ignored as shallower than the depth-10 line before).
    deep.emit('info depth 5 multipv 1 score cp -3 pv g1f3')
    expect(merged.length).toBe(beforeSwitch + 1)
    expect(merged[merged.length - 1]).toMatchObject({
      fen: nextFen,
      depth: 5,
      cp: -3,
    })
  })

  it('setWideEnabled(false) parks the wide engine and drops it from the merge', async () => {
    const deep = fakePort()
    const wide = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: deep.port, wide: wide.port },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()

    deep.emit('info depth 30 multipv 1 score cp 42 pv e2e4')
    wide.emit('info depth 20 multipv 2 score cp 10 pv d2d4')
    const beforeToggle = merged.length

    wide.sent.length = 0
    await session.setWideEnabled(false)

    // Wide engine is parked via UCI stop.
    expect(wide.sent).toContain('stop')

    // Merge no longer includes the wide slot.
    const lastOff = merged[merged.length - 1]
    expect(lastOff.lines).toEqual([{ multipv: 1, cp: 42, pv: ['e2e4'] }])
    expect(merged.length).toBe(beforeToggle + 1) // a re-merge was emitted

    // While parked, info from the wide engine is ignored.
    const beforeNoise = merged.length
    wide.emit('info depth 25 multipv 3 score cp -2 pv c2c4')
    expect(merged.length).toBe(beforeNoise)
  })

  it('setWideEnabled(true) resumes the wide engine on the current fen', async () => {
    const deep = fakePort()
    const wide = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: deep.port, wide: wide.port },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()
    await session.setWideEnabled(false)
    wide.sent.length = 0

    await session.setWideEnabled(true)

    // Wide engine is told the current fen and to go infinite again.
    expect(wide.sent).toEqual([`position fen ${START_FEN}`, 'go infinite'])

    // Info from wide is now re-incorporated.
    wide.emit('info depth 20 multipv 2 score cp 7 pv d2d4')
    const last = merged[merged.length - 1]
    expect(last.lines.some((l) => l.multipv === 2)).toBe(true)
  })

  it('applyHeavyResources reconfigures only the deep engine', async () => {
    const deep = fakePort()
    const wide = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      { deep: deep.port, wide: wide.port },
      { fen: START_FEN },
      { onMerge: (pos) => merged.push(pos) },
    )
    await session.start()
    deep.sent.length = 0
    wide.sent.length = 0

    await session.applyHeavyResources(4, 512)

    // Deep gets stop + setoption + position + go, in order.
    expect(deep.sent).toEqual([
      'stop',
      'setoption name Threads value 4',
      'setoption name Hash value 512',
      `position fen ${START_FEN}`,
      'go infinite',
    ])
    // Wide is untouched.
    expect(wide.sent).toEqual([])
  })

  it('persists the refined position to cache with a depth throttle', async () => {
    const deep = fakePort()
    const puts = vi.fn().mockResolvedValue(undefined)
    const cache = {
      putInfinite: puts,
    } as unknown as PositionCache
    const session = createLiveEvalSession(
      { deep: deep.port, wide: null },
      { fen: START_FEN },
      { onMerge: () => {} },
      { cache },
    )

    await session.start()

    deep.emit('info depth 1 multipv 1 score cp 5 pv e2e4') // first save at any depth
    deep.emit('info depth 4 multipv 1 score cp 6 pv e2e4') // +3 < 5 → skip
    deep.emit('info depth 7 multipv 1 score cp 7 pv e2e4') // +6 from 1 → save
    deep.emit('info depth 9 multipv 1 score cp 8 pv e2e4') // +2 from 7 → skip
    deep.emit('info depth 13 multipv 1 score cp 9 pv e2e4') // +6 from 7 → save

    expect(puts).toHaveBeenCalledTimes(3)
    const lastCall = puts.mock.calls[puts.mock.calls.length - 1]
    expect((lastCall[0] as RawPosition).depth).toBe(13)
  })
})
