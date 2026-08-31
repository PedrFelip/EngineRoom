import { describe, expect, it, vi } from 'vitest'
import { createReviewStore } from './review-store'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const AFTER_E4 = 'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
const AFTER_E5 = 'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2'

function mainlineMove(
  ply: number,
  color: 'w' | 'b',
  san: string,
  uci: string,
  fenBefore: string,
) {
  return {
    ply,
    color,
    san,
    uci,
    fenBefore,
    classification: 'melhor' as const,
    winPctBefore: 50,
    winPctAfter: 50,
    winPctLoss: 0,
    cpLoss: 0,
    bestUci: uci,
    isBook: false,
    eco: null,
  }
}

/** ReviewResult mínimo de '1. e4 e5' — fonte independente de FENs conhecidas. */
function e4e5Result() {
  const positions = [START_FEN, AFTER_E4, AFTER_E5].map((fen, i) => ({
    ply: i,
    fen,
    phase: 'opening' as const,
    depth: 20,
    cp: 0,
    winPct: 50,
    pv: ['e2e4'],
    lines: [],
  }))
  return {
    positions,
    moves: [
      mainlineMove(1, 'w', 'e4', 'e2e4', START_FEN),
      mainlineMove(2, 'b', 'e5', 'e7e5', AFTER_E4),
    ],
    accuracy: { white: 100, black: 100 },
    accuracyByPhase: {
      opening: { white: 100, black: 100 },
      middlegame: { white: 100, black: 100 },
      endgame: { white: 100, black: 100 },
    },
  }
}

describe('createReviewStore — resultado e snapshot', () => {
  it('setResult posiciona no último lance e expõe o resultado no snapshot', () => {
    const store = createReviewStore()
    const result = e4e5Result()

    store.setResult(result)

    const snap = store.getSnapshot()
    expect(snap.result).toBe(result)
    expect(snap.currentPly).toBe(2)
  })

  it('notifica assinantes em cada transição, e o snapshot fica estável entre elas', () => {
    const store = createReviewStore()
    const listener = vi.fn()
    const unsubscribe = store.subscribe(listener)

    store.setResult(e4e5Result())
    const snap = store.getSnapshot()
    store.goTo(1)

    expect(listener).toHaveBeenCalledTimes(2)
    expect(store.getSnapshot()).not.toBe(snap)
    unsubscribe()
    store.goTo(0)
    expect(listener).toHaveBeenCalledTimes(2)
  })
})

describe('createReviewStore — navegação na linha principal', () => {
  it('next avança e para no último ply; prev recua e para em 0', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)

    store.next()
    expect(store.getSnapshot().currentPly).toBe(1)
    store.next()
    store.next()
    expect(store.getSnapshot().currentPly).toBe(2)

    store.prev()
    expect(store.getSnapshot().currentPly).toBe(1)
    store.prev()
    store.prev()
    expect(store.getSnapshot().currentPly).toBe(0)
  })

  it('first e last levam ao início e ao fim', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())

    store.first()
    expect(store.getSnapshot().currentPly).toBe(0)
    store.last()
    expect(store.getSnapshot().currentPly).toBe(2)
  })

  it('goTo clamp dentro dos limites', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())

    store.goTo(-5)
    expect(store.getSnapshot().currentPly).toBe(0)
    store.goTo(99)
    expect(store.getSnapshot().currentPly).toBe(2)
  })
})
