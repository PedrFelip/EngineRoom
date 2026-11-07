import { Exit, Schema } from 'effect'
import { describe, expect, it } from 'vitest'
import {
  BResultSchema,
  ReviewConfigSchema,
  SessionEventSchema,
} from './review-protocol'
import { selectBDecisionPosition, selectDisplayedFen } from './review-selectors'
import { createReviewStore } from './review-store'

const result = {
  version: 1,
  pipeline: 'experimental-b',
  persistence: 'temporary',
  status: 'unresolved',
  positions: [
    { ply: 0, fen: 'start' },
    { ply: 1, fen: 'next' },
  ],
  decisions: [
    {
      ply: 1,
      version: 2,
      status: 'unresolved',
      best: { kind: 'cp', value: 20 },
      played: null,
      bestUci: 'e2e4',
      lines: [],
      reason: 'missing evidence',
    },
  ],
} as const

describe('experimental B protocol', () => {
  it('accepts a saved B result in a reopened review configuration', () => {
    const config = Schema.decodeUnknownSync(ReviewConfigSchema)({
      pgn: '1. e4 *',
      pipeline: 'experimental-b',
      meta: {
        white: 'W',
        black: 'B',
        whiteElo: null,
        blackElo: null,
        result: '*',
        event: null,
        plies: 1,
      },
      engine: { id: 'custom', label: 'Saved', depth: 8, hint: '' },
      mode: 'depth',
      analysisKind: 'manual',
      lines: 2,
      initialBResult: result,
    })
    expect(config.initialBResult?.decisions[0].reason).toBe('missing evidence')
    expect(config.pipeline).toBe('experimental-b')
  })
  it('decodes flat progress and unresolved results, rejecting invalid values', () => {
    expect(
      Exit.isSuccess(
        Schema.decodeUnknownExit(SessionEventSchema)({
          sessionId: 's',
          sequence: 1,
          type: 'bProgress',
          stage: 'coverage',
          completed: 1,
          total: 2,
        }),
      ),
    ).toBe(true)
    expect(
      Exit.isSuccess(
        Schema.decodeUnknownExit(SessionEventSchema)({
          sessionId: 's',
          sequence: 2,
          type: 'bCompleted',
          result,
        }),
      ),
    ).toBe(true)
    expect(
      Exit.isFailure(
        Schema.decodeUnknownExit(BResultSchema)({ ...result, version: 2 }),
      ),
    ).toBe(true)
  })
  it('keeps B separate and navigates its positions', () => {
    const decoded = Schema.decodeUnknownSync(BResultSchema)(result)
    const store = createReviewStore()
    store.setBResult(decoded)
    expect(store.getState().result).toBeNull()
    store.last()
    expect(store.getState().currentPly).toBe(1)
    expect(selectDisplayedFen(store.getState())).toBe('next')
    expect(selectBDecisionPosition(store.getState())?.fen).toBe('start')
    store.setBResult(decoded)
    expect(store.getState().currentPly).toBe(1)
    store.prev()
    expect(store.getState().currentPly).toBe(0)
    expect(store.getState().bResult?.decisions[0].reason).toBe(
      'missing evidence',
    )
  })
})
