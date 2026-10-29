import { Schema } from 'effect'
import { describe, expect, it } from 'vitest'
import reviewFixture from '../../lib/__tests__/fixtures/review.json'
import { ReviewResultSchema } from '../../lib/review-protocol'
import {
  selectDisplayedFen,
  selectDisplayedPosition,
} from '../../lib/review-selectors'
import { createReviewStore } from '../../lib/review-store'
import { EVAL_UNAVAILABLE_LABEL, evalBarView } from './eval-bar-view'

const existingResult = () =>
  Schema.decodeUnknownSync(ReviewResultSchema)(reviewFixture.result)

describe('barra de avaliação da posição exibida', () => {
  it('mantém a avaliação da linha principal e das linhas preliminares', () => {
    const result = existingResult()
    const mainline = result.positions[1]
    expect(evalBarView(mainline, true)).toEqual({
      winPct: mainline.winPct,
      label: '+0.5',
    })

    const preliminary = {
      ...result.positions[0],
      triageLines: result.positions[0].lines,
    }
    expect(evalBarView(preliminary, true)).toMatchObject({
      winPct: preliminary.winPct,
      label: expect.not.stringMatching(EVAL_UNAVAILABLE_LABEL),
    })
  })

  it('não reaproveita a nota anterior quando a variação ainda não tem avaliação', () => {
    const store = createReviewStore()
    store.setResult(existingResult())
    store.first()
    store.makeMove('d2', 'd4')

    const state = store.getState()
    expect(selectDisplayedPosition(state)).toBeNull()
    expect(evalBarView(selectDisplayedPosition(state), true)).toEqual({
      winPct: 50,
      label: EVAL_UNAVAILABLE_LABEL,
    })

    const fen = selectDisplayedFen(state)
    if (!fen) throw new Error('FEN da variação esperado')
    const live = {
      ...existingResult().positions[0],
      fen,
      cp: -80,
      winPct: 61,
    }
    store.setLiveAnalysis(fen, live)
    expect(selectDisplayedPosition(store.getState())).toBe(live)
    expect(
      evalBarView(selectDisplayedPosition(store.getState()), true),
    ).toEqual({ winPct: 61, label: '+0.8' })
  })

  it('mostra indisponível fora de uma revisão concluída', () => {
    expect(evalBarView(undefined, false)).toEqual({
      winPct: 50,
      label: undefined,
    })
  })
})
