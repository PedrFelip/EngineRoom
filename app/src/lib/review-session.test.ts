import { Deferred, Effect, Layer, Schema } from 'effect'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewConfig } from '../types'
import reviewFixture from './__tests__/fixtures/review.json'
import { AnalysisSessions, type SessionPort } from './backend'
import { SessionError } from './effect/errors'
import { mountReviewSession, runUi } from './effect/ui-runtime'
import {
  BResultSchema,
  ReviewResultSchema,
  type SessionEvent,
} from './review-protocol'
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
function fixture(initial = false, cleanup: Effect.Effect<void> = Effect.void) {
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
          }).pipe(Effect.andThen(cleanup)),
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
  it('preserves B navigation across revisions and rejects replaced live results', async () => {
    const bResult = Schema.decodeUnknownSync(BResultSchema)({
      version: 1,
      pipeline: 'experimental-b',
      persistence: 'temporary',
      status: 'unresolved',
      positions: result.positions
        .slice(0, 2)
        .map(({ fen }, ply) => ({ ply, fen })),
      decisions: [
        {
          ply: 1,
          version: 1,
          status: 'preliminary',
          best: null,
          played: null,
          bestUci: null,
          lines: [],
          reason: null,
        },
      ],
    })
    let emit: ((event: SessionEvent) => void) | undefined
    const analyze = vi.fn<SessionPort['analyzePosition']>(() => Effect.void)
    const states = vi.fn()
    const store = createReviewStore()
    const layer = Layer.succeed(AnalysisSessions, {
      open: (_config, onEvent) =>
        Effect.sync(() => {
          emit = onEvent
          return { analyzePosition: analyze, cancelLive: () => Effect.void }
        }),
    })
    const mounted = mountReviewSession(
      {
        config: { ...config, pipeline: 'experimental-b' },
        store,
        onStateChange: states,
        onProgress: vi.fn(),
      },
      layer,
    )
    const starting = mounted.start()
    await vi.waitFor(() => expect(emit).toBeDefined())
    const send = (event: SessionEvent) => emit?.(event)
    send({ sessionId: 'b', sequence: 1, type: 'bUpdated', result: bResult })
    store.goTo(1)
    const revised = {
      ...bResult,
      decisions: [
        {
          ...bResult.decisions[0],
          version: 2,
          status: 'unresolved' as const,
          reason: 'missing evidence',
        },
      ],
    }
    send({ sessionId: 'b', sequence: 2, type: 'bUpdated', result: revised })
    expect(store.getState().currentPly).toBe(1)
    expect(store.getState().bResult?.decisions[0].version).toBe(2)
    send({ sessionId: 'b', sequence: 3, type: 'bCompleted', result: revised })
    await starting
    expect(store.getState().currentPly).toBe(1)
    expect(states).toHaveBeenCalledWith({ status: 'done', error: null })
    mounted.analyzePosition({ fen: result.positions[0].fen }, settings)
    await vi.waitFor(() => expect(analyze).toHaveBeenCalledOnce())
    mounted.analyzePosition({ fen: result.positions[1].fen }, settings)
    await vi.waitFor(() => expect(analyze).toHaveBeenCalledTimes(2))
    send({
      sessionId: 'b',
      sequence: 4,
      type: 'liveCompleted',
      requestId: analyze.mock.calls[0][0],
      fen: result.positions[0].fen,
      analysis: result.positions[0],
    })
    expect(
      store.getState().liveAnalysis.positions[result.positions[0].fen],
    ).toBeUndefined()
    await mounted.dispose()
  })
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
  it('waits for acknowledged cleanup without reporting cancellation as an error', async () => {
    const releasing = Effect.runSync(Deferred.make<void>())
    const releaseAllowed = Effect.runSync(Deferred.make<void>())
    const f = fixture(
      false,
      Effect.gen(function* () {
        yield* Deferred.succeed(releasing, undefined)
        yield* Deferred.await(releaseAllowed)
      }),
    )
    await f.mounted.start()
    let disposed = false
    const closing = f.mounted.dispose().then(() => {
      disposed = true
    })
    await runUi(Deferred.await(releasing))
    expect(disposed).toBe(false)
    expect(f.close).toHaveBeenCalledOnce()
    await runUi(Deferred.succeed(releaseAllowed, undefined))
    await closing
    expect(disposed).toBe(true)
    expect(f.states.mock.calls.map(([state]) => state.status)).not.toContain(
      'error',
    )
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
