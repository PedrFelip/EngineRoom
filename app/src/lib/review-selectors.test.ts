import { Schema } from 'effect'
import { describe, expect, it } from 'vitest'
import reviewFixture from './__tests__/fixtures/review.json'
import { ReviewResultSchema } from './review-protocol'
import {
  selectDisplayedPly,
  selectSourceFen,
  selectSourcePosition,
} from './review-selectors'
import { createReviewStore, selectActiveVariation } from './review-store'

const existingResult = () =>
  Schema.decodeUnknownSync(ReviewResultSchema)(reviewFixture.result)

describe('posição anterior de uma variação', () => {
  it('usa a base no primeiro lance, mas exige o pai nos lances seguintes', () => {
    const store = createReviewStore()
    const result = existingResult()
    store.setResult(result)
    store.first()
    store.makeMove('d2', 'd4')
    expect(selectSourcePosition(store.getState())).toBe(result.positions[0])

    const parent = selectActiveVariation(store.getState())?.roots[0]
    if (!parent) throw new Error('Pai esperado')
    store.makeMove('d7', 'd5')
    expect(selectSourceFen(store.getState())).toBe(parent.fen)
    expect(selectSourcePosition(store.getState())).toBeUndefined()

    const analysis = { ...result.positions[0], fen: parent.fen, cp: 80 }
    store.setLiveAnalysis(parent.fen, analysis)
    expect(selectSourcePosition(store.getState())).toBe(analysis)
  })

  it('não retorna análise com FEN incompatível, mesmo indexada pelo pai', () => {
    const store = createReviewStore()
    const result = existingResult()
    store.setResult(result)
    store.first()
    store.exploreLine(['d2d4', 'd7d5'])
    store.next()
    const fen = selectSourceFen(store.getState())
    if (!fen) throw new Error('FEN do pai esperado')
    store.setLiveAnalysis(fen, result.positions[0])
    expect(selectSourcePosition(store.getState())).toBeUndefined()
  })
})

describe('ply exibido', () => {
  it('acompanha a navegação na linha principal e em variações', () => {
    const store = createReviewStore()
    store.setResult(existingResult())
    store.goTo(2)
    expect(selectDisplayedPly(store.getState())).toBe(2)

    store.makeMove('d2', 'd4')
    expect(store.getState().currentPly).toBe(2)
    expect(selectDisplayedPly(store.getState())).toBe(3)
    store.makeMove('d7', 'd5')
    expect(selectDisplayedPly(store.getState())).toBe(4)
    store.prev()
    expect(selectDisplayedPly(store.getState())).toBe(3)
    store.goTo(1)
    expect(selectDisplayedPly(store.getState())).toBe(1)
  })
})
