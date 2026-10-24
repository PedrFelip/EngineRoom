import { Effect, Fiber, Schema } from 'effect'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  ChessComSearch,
  SearchResultSchema,
  TauriChessComSearch,
} from './chess-com-import'

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

const page = {
  items: [
    {
      url: 'https://www.chess.com/game/live/123',
      endTime: 1700000000,
      timeClass: null,
      white: { username: 'Alice', rating: 1500 },
      black: { username: 'Bob', rating: null },
      result: '1-0',
      pgn: '[White "Alice"]\n[Black "Bob"]\n\n1. e4 e5 1-0',
    },
  ],
  nextCursor: {
    username: 'alice',
    year: 2026,
    month: 9,
    before: { endTime: 1700000000, url: 'https://www.chess.com/game/live/123' },
  },
  archivesRead: 1,
  skippedInvalid: 0,
  skippedUnsupported: 0,
  stopReason: 'page' as const,
  warning: null,
}
function search() {
  return Effect.gen(function* () {
    const service = yield* ChessComSearch
    return yield* service.search('alice', null)
  }).pipe(Effect.provide(TauriChessComSearch))
}

describe('Chess.com IPC scope', () => {
  beforeEach(() => {
    mocks.channels.length = 0
    mocks.invoke.mockReset().mockResolvedValue(undefined)
  })

  it('validates the Rust contract, including partial results and nullable fields', () => {
    const decode = Schema.decodeUnknownSync(SearchResultSchema)
    expect(decode(page)).toEqual(page)
    expect(
      decode({
        ...page,
        stopReason: 'error',
        warning: {
          code: 'rateLimited',
          message: 'Aguarde.',
          retryAfterSeconds: 60,
        },
      }).warning?.code,
    ).toBe('rateLimited')
    expect(() =>
      decode({ ...page, items: [{ ...page.items[0], pgn: 5 }] }),
    ).toThrow()
    expect(() =>
      decode({ ...page, items: [{ ...page.items[0], endTime: -1 }] }),
    ).toThrow()
    expect(() =>
      decode({ ...page, items: [{ ...page.items[0], endTime: 1e15 }] }),
    ).toThrow()
  })

  it('awaits acquisition before cancellation and awaits Rust cleanup exactly once', async () => {
    let acknowledge: () => void = () => {}
    let finishCleanup: () => void = () => {}
    mocks.invoke.mockImplementation(
      (command) =>
        new Promise<void>((resolve) => {
          if (command === 'chess_com_search_start') acknowledge = resolve
          else finishCleanup = resolve
        }),
    )
    const fiber = Effect.runFork(search())
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledOnce())
    let finished = false
    const closing = Effect.runPromise(Fiber.interrupt(fiber)).then(() => {
      finished = true
    })
    expect(mocks.invoke.mock.calls).toHaveLength(1)
    acknowledge()
    await vi.waitFor(() => expect(mocks.invoke.mock.calls).toHaveLength(2))
    expect(finished).toBe(false)
    finishCleanup()
    await closing
    expect(mocks.invoke.mock.calls.map(([command]) => command)).toEqual([
      'chess_com_search_start',
      'chess_com_search_cancel',
    ])
  })

  it('ignores other request IDs, delivers a selected PGN and closes the task', async () => {
    const fiber = Effect.runFork(search())
    await vi.waitFor(() => expect(mocks.channels).toHaveLength(1))
    const requestId = mocks.invoke.mock.calls[0][1]?.requestId
    const channel = mocks.channels[0]
    channel.onmessage({ requestId: 'other', type: 'completed', result: {} })
    channel.onmessage({ requestId, type: 'completed', result: page })
    const result = await Effect.runPromise(Fiber.join(fiber))
    expect(result.items[0].pgn).toBe(page.items[0].pgn)
    expect(mocks.invoke).toHaveBeenLastCalledWith('chess_com_search_cancel', {
      requestId,
    })
  })

  it('fails malformed events and preserves structured remote errors', async () => {
    for (const payload of [
      { type: 'completed', result: {} },
      {
        type: 'error',
        error: {
          code: 'rateLimited',
          message: 'Aguarde.',
          retryAfterSeconds: 90,
        },
      },
    ]) {
      const fiber = Effect.runFork(search())
      await vi.waitFor(() => expect(mocks.channels).toHaveLength(1))
      const requestId = mocks.invoke.mock.calls[0][1]?.requestId
      mocks.channels[0].onmessage({ requestId, ...payload })
      await expect(Effect.runPromise(Fiber.join(fiber))).rejects.toThrow()
      expect(mocks.invoke.mock.calls.map(([command]) => command)).toContain(
        'chess_com_search_cancel',
      )
      mocks.channels.length = 0
      mocks.invoke.mockClear()
    }
  })
})
