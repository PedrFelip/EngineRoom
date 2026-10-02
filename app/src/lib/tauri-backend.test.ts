import { Effect, Fiber, Schema } from 'effect'
import { describe, expect, it, vi } from 'vitest'
import reviewFixture from './__tests__/fixtures/review.json'
import { AnalysisSessions } from './backend'
import { LegacyReviewSchema } from './effect/schemas'
import { ProbeResultSchema, SessionEventSchema } from './review-protocol'
import { TauriBackend } from './tauri-backend'

const mocks = vi.hoisted(() => ({
  invoke:
    vi.fn<
      (command: string, args?: Record<string, unknown>) => Promise<unknown>
    >(),
  channels: [] as Array<{ onmessage: (payload: unknown) => void }>,
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
  Channel: class {
    onmessage = (_payload: unknown) => {}
    constructor() {
      mocks.channels.push(this)
    }
  },
}))
const config = {
  pgn: '1. e4',
  meta: {
    white: 'w',
    black: 'b',
    whiteElo: null,
    blackElo: null,
    event: null,
    result: '*',
    plies: 1,
  },
  engine: { id: 'balanced' as const, depth: 20, label: '', hint: '' },
  mode: 'depth' as const,
  lines: 1,
}

describe('session IPC acquisition and validation', () => {
  it('preserves preliminary lines and depth separately through IPC and history', () => {
    const result = structuredClone(reviewFixture.result)
    const position = {
      ...result.positions[0],
      lines: result.positions[0].lines.slice(0, 1),
      triageLines: result.positions[0].lines.map((line) => ({
        ...line,
        depth: 12,
      })),
    }
    const review = {
      ...result,
      positions: [position, ...result.positions.slice(1)],
    }
    const event = Schema.decodeUnknownSync(SessionEventSchema)({
      type: 'completed',
      sessionId: 'session',
      sequence: 1,
      result: review,
    })
    if (event.type !== 'completed') throw new Error('expected completed')
    expect(event.result.positions[0].triageLines).toEqual(position.triageLines)
    expect(event.result.positions[0].lines).toHaveLength(1)
    const history = Schema.decodeUnknownSync(LegacyReviewSchema)(
      JSON.stringify(review),
    )
    expect(history.positions[0].triageLines).toEqual(position.triageLines)
  })
  it('waits for open acknowledgement before honoring disposal and closes exactly once', async () => {
    mocks.channels.length = 0
    let finish: () => void = () => {}
    mocks.invoke.mockReset().mockImplementation((command) =>
      command === 'review_session_open'
        ? new Promise<void>((resolve) => {
            finish = resolve
          })
        : Promise.resolve(),
    )
    const root = Effect.runFork(
      Effect.scoped(
        Effect.gen(function* () {
          const service = yield* AnalysisSessions
          yield* service.open(
            config,
            () => {},
            () => {},
          )
          yield* Effect.never
        }),
      ).pipe(Effect.provide(TauriBackend)),
    )
    await vi.waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith(
        'review_session_open',
        expect.anything(),
      ),
    )
    const closing = Effect.runPromise(Fiber.interrupt(root))
    await Promise.resolve()
    expect(
      mocks.invoke.mock.calls.filter(([c]) => c === 'review_session_close'),
    ).toHaveLength(0)
    finish()
    await closing
    expect(
      mocks.invoke.mock.calls.filter(([c]) => c === 'review_session_close'),
    ).toHaveLength(1)
  })
  it('ignores another session and duplicate sequences, then closes on invalid data', async () => {
    mocks.channels.length = 0
    mocks.invoke.mockReset().mockResolvedValue(undefined)
    const events = vi.fn()
    const errors = vi.fn()
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const service = yield* AnalysisSessions
          yield* service.open(config, events, errors)
          const id = mocks.invoke.mock.calls[0]?.[1]?.sessionId
          const channel = mocks.channels[0]
          const event = {
            type: 'completed',
            result: reviewFixture.result,
            sessionId: id,
            sequence: 1,
          }
          channel.onmessage({ ...event, sessionId: 'other' })
          channel.onmessage(event)
          channel.onmessage(event)
          expect(events).toHaveBeenCalledOnce()
          channel.onmessage({
            ...event,
            sequence: 2,
            result: { invalid: true },
          })
          expect(errors).toHaveBeenCalledOnce()
          yield* Effect.yieldNow()
          channel.onmessage({ ...event, sequence: 3 })
          expect(events).toHaveBeenCalledOnce()
        }),
      ).pipe(Effect.provide(TauriBackend)),
    )
    expect(
      mocks.invoke.mock.calls.some(([c]) => c === 'review_session_close'),
    ).toBe(true)
  })
  it('validates payloads as sent by Rust, including tagged errors and optional fields', () => {
    const decode = Schema.decodeUnknownSync(SessionEventSchema)
    expect(
      decode({
        sessionId: 'id',
        sequence: 1,
        type: 'completed',
        result: reviewFixture.result,
      }).type,
    ).toBe('completed')
    expect(
      decode({
        sessionId: 'id',
        sequence: 2,
        requestId: 3,
        type: 'error',
        fen: 'fen',
        error: {
          code: 'engineTimeout',
          operation: 'search',
          message: 'timeout',
        },
      }).type,
    ).toBe('error')
    expect(() =>
      decode({
        sessionId: 'id',
        sequence: 1,
        type: 'completed',
        result: { accuracy: { white: Number.NaN, black: 1 } },
      }),
    ).toThrow()
    expect(
      Schema.decodeUnknownSync(ProbeResultSchema)({
        ok: true,
        name: 'Stockfish 18',
      }),
    ).toEqual({ ok: true, name: 'Stockfish 18' })
  })
})
