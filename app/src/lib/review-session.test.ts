import { Effect, Layer, Schema } from 'effect'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewConfig } from '../types'
import reviewFixture from './__tests__/fixtures/review.json'
import { AnalysisSessions, type SessionPort } from './backend'
import { SessionError } from './effect/errors'
import { mountReviewSession } from './effect/ui-runtime'
import { ReviewResultSchema, type SessionEvent } from './review-protocol'
import type { LiveAnalysisSettings } from './review-session'
import { createReviewStore } from './review-store'

const result = Schema.decodeUnknownSync(ReviewResultSchema)(
  reviewFixture.result,
)
const config: ReviewConfig = {
  pgn: reviewFixture.pgn,
  meta: {
    white: 'Jogador',
    black: 'Jogador',
    whiteElo: null,
    blackElo: null,
    result: '*',
    event: null,
    plies: result.moves.length,
  },
  engine: { id: 'balanced', depth: 20, label: 'Equilibrado', hint: '' },
  mode: 'depth',
  lines: 3,
}
const settings: LiveAnalysisSettings = {
  searchSeconds: 1,
  lines: 3,
  threadsAuto: false,
  threads: 1,
  memoryMb: 16,
  moveFeedbackEnabled: false,
}
function fixture(initial = false) {
  let emit: (event: SessionEvent) => void = () => {}
  let sequence = 0
  type Payload<T> = T extends SessionEvent
    ? Omit<T, 'sequence' | 'sessionId'>
    : never
  const send = (event: Payload<SessionEvent>) => {
    emit({ ...event, sequence: ++sequence, sessionId: 'test' } as SessionEvent)
  }
  const analyze = vi.fn<SessionPort['analyzePosition']>(() => Effect.void)
  const cancel = vi.fn<SessionPort['cancelLive']>(() => Effect.void)
  const close = vi.fn()
  const states = vi.fn()
  const progress = vi.fn()
  const store = createReviewStore()
  const layer = Layer.succeed(AnalysisSessions, {
    open: (_config, onEvent) =>
      Effect.acquireRelease(
        Effect.sync(() => {
          emit = onEvent
          send({ type: 'completed', result })
          return { analyzePosition: analyze, cancelLive: cancel }
        }),
        () =>
          Effect.sync(() => {
            close()
          }),
      ),
  })
  const mounted = mountReviewSession(
    {
      config: initial ? { ...config, initialResult: result } : config,
      store,
      onStateChange: states,
      onProgress: progress,
    },
    layer,
  )
  return { mounted, store, states, progress, analyze, cancel, close, send }
}

describe('Rust session UI glue', () => {
  it('applies the completed review and releases the session once', async () => {
    const f = fixture()
    await f.mounted.start()
    await f.mounted.start()
    expect(f.store.getState().result).toEqual(result)
    expect(f.states).toHaveBeenCalledWith({ status: 'done', error: null })
    await f.mounted.dispose()
    await f.mounted.dispose()
    expect(f.close).toHaveBeenCalledOnce()
  })
  it('restores immediately and preserves navigation when backend confirms it', async () => {
    const f = fixture(true)
    const started = f.mounted.start()
    expect(f.store.getState().result).toBe(result)
    f.store.goTo(1)
    await started
    expect(f.store.getState().currentPly).toBe(1)
    await f.mounted.dispose()
  })
  it('retains early intent and ignores results and errors for replaced requests', async () => {
    const f = fixture()
    const started = f.mounted.start()
    const first = result.positions[0]
    const second = result.positions[1]
    f.mounted.analyzePosition({ fen: first.fen }, settings)
    await started
    await vi.waitFor(() => expect(f.analyze).toHaveBeenCalledOnce())
    f.mounted.analyzePosition({ fen: second.fen }, settings)
    await vi.waitFor(() => expect(f.analyze).toHaveBeenCalledTimes(2))
    const oldId = f.analyze.mock.calls[0]?.[0]
    const currentId = f.analyze.mock.calls[1]?.[0]
    f.send({
      type: 'liveCompleted',
      requestId: oldId,
      fen: first.fen,
      analysis: first,
    })
    f.send({
      type: 'error',
      requestId: oldId,
      fen: first.fen,
      error: { code: 'engineExited', operation: 'search', message: 'stale' },
    })
    expect(f.store.getState().liveAnalysis.positions[first.fen]).toBeUndefined()
    f.send({
      type: 'liveCompleted',
      requestId: currentId,
      fen: second.fen,
      analysis: second,
    })
    expect(f.store.getState().liveAnalysis.positions[second.fen]).toEqual(
      second,
    )
    await f.mounted.dispose()
    f.send({ type: 'completed', result })
    expect(f.close).toHaveBeenCalledOnce()
  })
  it('cancels visual state immediately and rejects late results', async () => {
    const f = fixture()
    await f.mounted.start()
    const position = result.positions[0]
    f.mounted.analyzePosition({ fen: position.fen }, settings)
    await vi.waitFor(() => expect(f.analyze).toHaveBeenCalledOnce())
    f.mounted.cancelLiveAnalysis()
    expect(f.store.getState().liveAnalysis.status).toBe('cancelled')
    f.send({
      type: 'liveCompleted',
      requestId: 1,
      fen: position.fen,
      analysis: position,
    })
    expect(f.store.getState().liveAnalysis.positions).toEqual({})
    await vi.waitFor(() => expect(f.cancel).toHaveBeenCalledOnce())
    await f.mounted.dispose()
  })
  it('accumulates indexed progress without performing analysis in the UI', async () => {
    const f = fixture()
    await f.mounted.start()
    for (const [index, p] of result.positions.entries()) {
      f.send({
        type: 'progress',
        progress: {
          stage: 'triage',
          completed: index + 1,
          total: result.positions.length,
          currentPly: index,
          phase: p.phase,
          cachedPositions: 0,
          enginePositions: index + 1,
          update: { index, winPct: p.winPct },
        },
      })
    }
    expect(f.progress.mock.lastCall?.[0].winPcts).toEqual(
      result.positions.map((p) => p.winPct),
    )
    expect(f.analyze).not.toHaveBeenCalled()
    await f.mounted.dispose()
  })
  it('surfaces acquisition failures and completes startup', async () => {
    const onStateChange = vi.fn()
    const mounted = mountReviewSession(
      {
        config,
        store: createReviewStore(),
        onStateChange,
        onProgress: () => {},
      },
      Layer.succeed(AnalysisSessions, {
        open: () =>
          Effect.fail(
            new SessionError({
              code: 'engineSpawn',
              operation: 'open',
              message: 'missing engine',
            }),
          ),
      }),
    )
    await mounted.start()
    expect(onStateChange).toHaveBeenCalledWith({
      status: 'error',
      error: 'missing engine',
    })
    await mounted.dispose()
  })
})
