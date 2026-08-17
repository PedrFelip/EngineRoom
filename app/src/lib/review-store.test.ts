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
    expect(snap.variations).toEqual({})
    expect(snap.currentVariation).toBeNull()
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

  it('getDisplayedFen devolve o FEN da posição atual da linha principal', () => {
    const store = createReviewStore()
    expect(store.getDisplayedFen()).toBeNull()

    store.setResult(e4e5Result())
    store.goTo(1)
    expect(store.getDisplayedFen()).toBe(AFTER_E4)
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
})

describe('createReviewStore — makeMove na linha principal', () => {
  it('lance que coincide com o próximo da linha principal apenas avança', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)

    store.makeMove('e2e4')

    const snap = store.getSnapshot()
    expect(snap.currentPly).toBe(1)
    expect(snap.currentVariation).toBeNull()
    expect(snap.variations).toEqual({})
  })

  it('lance divergente abre variação ramificada do ply atual e a foca', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(1)

    store.makeMove('c7c5')

    const snap = store.getSnapshot()
    expect(snap.currentPly).toBe(1)
    const list = snap.variations[1]
    expect(list).toHaveLength(1)
    const v = list[0]
    expect(v.parentPly).toBe(1)
    expect(v.moves).toHaveLength(1)
    const m = v.moves[0]
    expect(m.san).toBe('c5')
    expect(m.uci).toBe('c7c5')
    expect(m.ply).toBe(1)
    // Nasce pendente: sem nota/afterCp até o refino ao vivo preencher.
    expect(m.afterCp).toBeUndefined()
    expect(m.classification).toBeUndefined()
    expect(snap.currentVariation).toEqual({
      variationId: v.id,
      parentPly: 1,
      ply: 1,
    })
  })

  it('lance ilegal não tem efeito', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)

    store.makeMove('e2e5')

    expect(store.getSnapshot().currentPly).toBe(0)
    expect(store.getSnapshot().variations).toEqual({})
  })

  it('lance sem resultado instalado não tem efeito', () => {
    const store = createReviewStore()
    store.makeMove('e2e4')
    expect(store.getSnapshot().variations).toEqual({})
  })
})

describe('createReviewStore — navegação em variações', () => {
  /** Store com uma variação '1... c5 2. Nf3' ramificada do ply 1. */
  function storeWithVariation() {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(1)
    store.makeMove('c7c5') // variação ramificada do ply 1, lance 1
    store.makeMove('g1f3') // segundo lance da variação
    return store
  }

  it('goToVariation foca a variação e sincroniza currentPly com o ply-pai', () => {
    const store = storeWithVariation()
    store.goTo(0)

    const v = store.getSnapshot().variations[1][0]
    store.goToVariation(v.id, 1, 2)

    const snap = store.getSnapshot()
    expect(snap.currentPly).toBe(1)
    expect(snap.currentVariation).toEqual({
      variationId: v.id,
      parentPly: 1,
      ply: 2,
    })
    expect(store.getDisplayedFen()).toBe(v.moves[1].fenAfter)
  })

  it('next dentro da variação avança até o último lance e para (não salta para a linha principal)', () => {
    const store = storeWithVariation()
    const v = store.getSnapshot().variations[1][0]
    store.goToVariation(v.id, 1, 1)

    store.next()
    expect(store.getSnapshot().currentVariation?.ply).toBe(2)
    store.next()
    store.next()
    expect(store.getSnapshot().currentVariation?.ply).toBe(2)
  })

  it('prev no primeiro lance da variação sai para a linha principal no ply-pai', () => {
    const store = storeWithVariation()
    const v = store.getSnapshot().variations[1][0]
    store.goToVariation(v.id, 1, 1)

    store.prev()

    const snap = store.getSnapshot()
    expect(snap.currentVariation).toBeNull()
    expect(snap.currentPly).toBe(1)
  })

  it('exitVariation volta à linha principal no ply-pai; goTo/first/last também saem da variação', () => {
    const store = storeWithVariation()
    const v = store.getSnapshot().variations[1][0]
    store.goToVariation(v.id, 1, 2)

    store.exitVariation()
    expect(store.getSnapshot().currentVariation).toBeNull()
    expect(store.getSnapshot().currentPly).toBe(1)

    store.goToVariation(v.id, 1, 1)
    store.goTo(0)
    expect(store.getSnapshot().currentVariation).toBeNull()

    store.goToVariation(v.id, 1, 1)
    store.last()
    expect(store.getSnapshot().currentVariation).toBeNull()
    expect(store.getSnapshot().currentPly).toBe(2)
  })

  it('makeMove no fim da variação anexa lance pendente e foca o novo ply', () => {
    const store = storeWithVariation()
    const v0 = store.getSnapshot().variations[1][0]

    store.makeMove('d7d6') // terceiro lance da variação

    const snap = store.getSnapshot()
    const v = snap.variations[1].find((x) => x.id === v0.id)
    expect(v?.moves).toHaveLength(3)
    expect(v?.moves[2].san).toBe('d6')
    expect(snap.currentVariation).toEqual({
      variationId: v0.id,
      parentPly: 1,
      ply: 3,
    })
  })

  it('makeMove no meio da variação trunca a cauda e insere no ply seguinte ao foco', () => {
    const store = storeWithVariation()
    const v0 = store.getSnapshot().variations[1][0]
    store.goToVariation(v0.id, 1, 1) // exibe a posição após c5, antes de Nf3

    store.makeMove('d2d4') // diverge do lance 2 existente (g1f3)

    const snap = store.getSnapshot()
    const v = snap.variations[1].find((x) => x.id === v0.id)
    expect(v?.moves).toHaveLength(2)
    expect(v?.moves[1].san).toBe('d4')
    expect(v?.moves[1].fenBefore).toBe(v?.moves[0].fenAfter)
    expect(v?.moves[1].ply).toBe(2)
    expect(snap.currentVariation).toEqual({
      variationId: v0.id,
      parentPly: 1,
      ply: 2,
    })
  })

  it('makeMove que coincide com o próximo lance da variação apenas avança o foco', () => {
    const store = storeWithVariation()
    const v0 = store.getSnapshot().variations[1][0]
    store.goToVariation(v0.id, 1, 1)

    store.makeMove('g1f3') // igual ao lance 2 existente

    const snap = store.getSnapshot()
    const v = snap.variations[1].find((x) => x.id === v0.id)
    expect(v?.moves).toHaveLength(2)
    expect(v?.moves[1].san).toBe('Nf3')
    expect(snap.currentVariation).toEqual({
      variationId: v0.id,
      parentPly: 1,
      ply: 2,
    })
  })
})

describe('createReviewStore — refino ao vivo', () => {
  function storeWithPendingVariation() {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(1)
    store.makeMove('c7c5')
    const snap = store.getSnapshot()
    const v = snap.variations[1][0]
    return { store, variation: v, moveId: v.moves[0].id }
  }

  it('getAnalysisTarget é mainline sem foco e variation com lance focado', () => {
    const { store, variation, moveId } = storeWithPendingVariation()
    // makeMove foca a variação recém-criada; saindo dela volta a mainline.
    store.exitVariation()
    expect(store.getAnalysisTarget()).toEqual({ kind: 'mainline' })

    store.goToVariation(variation.id, 1, 1)
    expect(store.getAnalysisTarget()).toEqual({
      kind: 'variation',
      variationId: variation.id,
      moveId,
    })
  })

  it('applyLive classifica o lance pendente usando o cp da linha principal', () => {
    const { store, variation, moveId } = storeWithPendingVariation()

    store.applyLive(
      { variationId: variation.id, moveId },
      {
        fen: 'qualquer',
        cp: -150, // pretas ficam piores após c5 neste cenário
        depth: 24,
        pv: ['g1f3', 'd7d6'],
        lines: [{ multipv: 1, cp: -150, pv: ['g1f3', 'd7d6'] }],
      },
    )

    const v = store.getSnapshot().variations[1][0]
    const m = v.moves[0]
    expect(m.afterCp).toBe(-150)
    expect(m.depth).toBe(24)
    expect(m.bestUci).toBe('g1f3')
    expect(m.classification).toBeDefined()
    expect(m.winPctBefore).toBe(50)
    expect(m.lines).toHaveLength(1)
    expect(m.lines?.[0].san).toBeNull()
  })

  it('applyLive com alvo inexistente não muda o snapshot', () => {
    const { store } = storeWithPendingVariation()
    const before = store.getSnapshot()

    store.applyLive(
      { variationId: 'v999', moveId: 'm999' },
      { fen: 'x', cp: 10, depth: 1, pv: [], lines: [] },
    )

    expect(store.getSnapshot()).toBe(before)
  })
})
