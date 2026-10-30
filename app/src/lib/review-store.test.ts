import { describe, expect, it, vi } from 'vitest'
import type { MoveAnalysis } from '../types'
import { selectDisplayedPosition } from './review-selectors'
import { createReviewStore, selectActiveVariation } from './review-store'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const AFTER_E4 = 'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
const AFTER_E5 = 'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2'

function mainlineMove(
  ply: number,
  color: 'w' | 'b',
  san: string,
  uci: string,
  fenBefore: string,
): { -readonly [K in keyof MoveAnalysis]: MoveAnalysis[K] } {
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
    accuracyModel: 'test',
    accuracy: { white: 100, black: 100 },
    accuracyByPhase: {
      opening: { white: 100, black: 100 },
      middlegame: { white: 100, black: 100 },
      endgame: { white: 100, black: 100 },
    },
  }
}

describe('createReviewStore — resultado e snapshot', () => {
  it('preserva seleções da tela quando só o status ao vivo muda', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    const before = store.getState()
    const position = selectDisplayedPosition(before)
    store.startLiveAnalysis(AFTER_E5)
    store.failLiveAnalysis(AFTER_E5, 'Falha de teste')
    const after = store.getState()
    expect(after.result).toBe(before.result)
    expect(after.variations).toBe(before.variations)
    expect(selectDisplayedPosition(after)).toBe(position)
  })

  it('mantém stores de revisões independentes', () => {
    const first = createReviewStore()
    const second = createReviewStore()
    first.setResult(e4e5Result())
    expect(second.getState().result).toBeNull()
    expect(first.getInitialState().result).toBeNull()
  })

  it('preserva a seleção ao reutilizar e classificar um filho existente', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.first()
    store.exploreLine(['d2d4', 'd7d5'])
    expect(store.makeMove('d7', 'd5')).toBe(true)
    const variation = selectActiveVariation(store.getSnapshot())
    if (!variation) throw new Error('Variação esperada após explorar linha')
    expect(store.getSnapshot().variations[0]).toBe(variation)
    store.setVariationClassification(variation.path[1], 'bom')
    expect(selectActiveVariation(store.getSnapshot())?.path).toEqual(
      variation.path,
    )
  })

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

  it('não notifica assinantes quando a navegação já está no destino', () => {
    const store = createReviewStore()
    const listener = vi.fn()
    store.setResult(e4e5Result())
    store.subscribe(listener)

    store.next()
    store.last()
    store.goTo(2)
    store.first()
    store.prev()
    store.goTo(0)

    expect(listener).toHaveBeenCalledTimes(1)
    expect(store.getSnapshot().currentPly).toBe(0)
  })
})

describe('createReviewStore — erros e blunders', () => {
  it('pula entre erros e blunders e permanece no limite', () => {
    const store = createReviewStore()
    const result = e4e5Result()
    result.moves[0].classification = 'erro'
    result.moves[1].classification = 'blunder'
    result.moves.push(
      { ...result.moves[1], ply: 3, classification: 'imprecisao' },
      { ...result.moves[1], ply: 4, san: 'Cf6', classification: 'erro' },
    )
    store.setResult(result)
    store.first()
    const listener = vi.fn()
    store.subscribe(listener)

    store.nextCritical()
    expect(store.getSnapshot().currentPly).toBe(1)
    store.nextCritical()
    expect(store.getSnapshot().currentPly).toBe(2)
    store.nextCritical()
    expect(store.getSnapshot().currentPly).toBe(4)
    store.nextCritical()
    expect(store.getSnapshot().currentPly).toBe(4)

    store.prevCritical()
    expect(store.getSnapshot().currentPly).toBe(2)
    store.prevCritical()
    expect(store.getSnapshot().currentPly).toBe(1)
    store.prevCritical()
    expect(store.getSnapshot().currentPly).toBe(1)
    expect(listener).toHaveBeenCalledTimes(5)
  })

  it('não avança quando a partida não tem erro nem blunder', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.first()
    const listener = vi.fn()
    store.subscribe(listener)

    store.nextCritical()
    store.prevCritical()

    expect(store.getSnapshot().currentPly).toBe(0)
    expect(listener).not.toHaveBeenCalled()
  })

  it('sai da variação para o erro de origem antes de recuar mais', () => {
    const store = createReviewStore()
    const result = e4e5Result()
    result.moves[0].classification = 'blunder'
    result.moves[1].classification = 'erro'
    store.setResult(result)
    store.goTo(1)
    expect(store.exploreLine(['d7d5'])).not.toEqual([])
    expect(selectActiveVariation(store.getSnapshot())).not.toBeNull()

    store.prevCritical()
    expect(selectActiveVariation(store.getSnapshot())).toBeNull()
    expect(store.getSnapshot().currentPly).toBe(1)

    store.exploreLine(['d7d5'])
    store.nextCritical()
    expect(selectActiveVariation(store.getSnapshot())).toBeNull()
    expect(store.getSnapshot().currentPly).toBe(2)
  })

  it('mantém a variação quando não há erro anterior', () => {
    const store = createReviewStore()
    const result = e4e5Result()
    result.moves[1].classification = 'erro'
    store.setResult(result)
    store.first()
    expect(store.exploreLine(['d2d4'])).not.toEqual([])

    store.prevCritical()
    expect(selectActiveVariation(store.getSnapshot())).not.toBeNull()
    expect(store.getSnapshot().currentPly).toBe(0)

    store.nextCritical()
    expect(selectActiveVariation(store.getSnapshot())).toBeNull()
    expect(store.getSnapshot().currentPly).toBe(2)
  })
})

describe('createReviewStore — ramificações locais', () => {
  it('reutiliza a raiz ao repetir um lance e preserva sua continuação e classificação', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.first()
    store.exploreLine(['e2e4', 'e7e5'])
    const initial = selectActiveVariation(store.getState())
    if (!initial) throw new Error('Variação esperada')
    const rootId = initial.roots[0].id
    store.setVariationClassification(rootId, 'bom')
    const before = selectActiveVariation(store.getState())
    if (!before) throw new Error('Variação esperada')
    store.prev()

    expect(store.makeMove('e2', 'e4')).toBe(true)

    const after = store.getState()
    expect(selectActiveVariation(after)?.roots).toBe(before.roots)
    expect(selectActiveVariation(after)?.roots).toHaveLength(1)
    expect(selectActiveVariation(after)?.roots[0].classification).toBe('bom')
    expect(selectActiveVariation(after)?.path).toEqual([rootId])
    expect(after.variations).toHaveLength(1)
    expect(after.variations[0]).toBe(selectActiveVariation(after))
    store.next()
    expect(selectActiveVariation(store.getState())?.path).toEqual([
      rootId,
      before.roots[0].children[0].id,
    ])
  })

  it('mantém raízes distintas e reutiliza filhos sem alterar snapshots anteriores', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.first()
    store.makeMove('e2', 'e4')
    const before = store.getState()
    store.prev()
    store.makeMove('d2', 'd4')
    store.makeMove('d7', 'd5')
    const branch = selectActiveVariation(store.getState())
    if (!branch) throw new Error('Variação esperada')
    store.prev()
    store.makeMove('d7', 'd5')

    expect(
      selectActiveVariation(before)?.roots.map((move) => move.uci),
    ).toEqual(['e2e4'])
    expect(selectActiveVariation(store.getState())?.roots).toBe(branch.roots)
    expect(branch.roots.map((move) => move.uci)).toEqual(['e2e4', 'd2d4'])
    expect(selectActiveVariation(store.getState())?.path).toEqual(branch.path)
    expect(store.getState().variations[0]).toBe(
      selectActiveVariation(store.getState()),
    )
  })

  it('cria uma ramificação com um lance legal e permite navegar nela', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)

    expect(store.makeMove('d2', 'd4')).toBe(true)
    expect(selectActiveVariation(store.getSnapshot())?.roots[0]?.san).toBe('d4')
    expect(selectActiveVariation(store.getSnapshot())?.path).toHaveLength(1)

    store.prev()
    expect(selectActiveVariation(store.getSnapshot())?.path).toHaveLength(0)
    store.next()
    expect(selectActiveVariation(store.getSnapshot())?.path).toEqual([
      selectActiveVariation(store.getSnapshot())?.roots[0]?.id,
    ])
    expect(selectActiveVariation(store.getSnapshot())?.roots[0]?.san).toBe('d4')
    store.prev()
    expect(selectActiveVariation(store.getSnapshot())?.path).toHaveLength(0)
    store.prev()
    expect(selectActiveVariation(store.getSnapshot())).toBeNull()
  })

  it('carrega uma PV e cria uma alternativa sem apagar sua continuação', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)
    store.exploreLine(['e2e4', 'e7e5', 'g1f3'])

    expect(
      selectActiveVariation(store.getSnapshot())?.roots[0]?.children[0]
        ?.children[0]?.san,
    ).toBe('Nf3')
    expect(selectActiveVariation(store.getSnapshot())?.path).toHaveLength(1)
    expect(store.makeMove('c7', 'c5')).toBe(true)
    const children = selectActiveVariation(store.getSnapshot())?.roots[0]
      ?.children
    expect(children?.map((move) => move.san)).toEqual(['e5', 'c5'])
  })

  it('anexa uma nova PV à posição atual de uma variação', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)
    store.makeMove('d2', 'd4')

    store.exploreLine(['d7d5', 'c2c4'])

    const root = selectActiveVariation(store.getSnapshot())?.roots[0]
    expect(root?.san).toBe('d4')
    expect(root?.children[0]?.san).toBe('d5')
    expect(root?.children[0]?.children[0]?.san).toBe('c4')
    expect(selectActiveVariation(store.getSnapshot())?.path).toEqual([
      root?.id,
      root?.children[0]?.id,
    ])
  })

  it('cria uma alternativa dentro de outra alternativa', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)
    store.exploreLine(['e2e4', 'e7e5'])
    expect(store.makeMove('c7', 'c5')).toBe(true)
    expect(store.makeMove('g1', 'f3')).toBe(true)
    store.prev()
    expect(store.makeMove('b1', 'c3')).toBe(true)

    const alternative = selectActiveVariation(store.getSnapshot())?.roots[0]
      ?.children[1]
    expect(alternative?.san).toBe('c5')
    expect(alternative?.children.map((move) => move.san)).toEqual([
      'Nf3',
      'Nc3',
    ])

    const nestedPath = selectActiveVariation(store.getSnapshot())?.path
    const variationId = selectActiveVariation(store.getSnapshot())?.id
    expect(nestedPath).toHaveLength(3)
    store.goToVariation(variationId ?? '', nestedPath?.slice(0, 2) ?? [])
    expect(selectActiveVariation(store.getSnapshot())?.path).toEqual(
      nestedPath?.slice(0, 2),
    )
  })

  it('preserva linhas alternativas ao navegar pela partida principal', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)
    store.makeMove('d2', 'd4')
    const savedId = selectActiveVariation(store.getSnapshot())?.id

    store.goTo(2)
    expect(selectActiveVariation(store.getSnapshot())).toBeNull()
    expect(store.getSnapshot().variations).toHaveLength(1)

    store.goToVariation(
      savedId ?? '',
      store.getSnapshot().variations[0]?.path ?? [],
    )
    expect(selectActiveVariation(store.getSnapshot())?.id).toBe(savedId)
    expect(store.getSnapshot().currentPly).toBe(0)
  })

  it('rejeita lances ilegais sem criar ramificação', () => {
    const store = createReviewStore()
    store.setResult(e4e5Result())
    store.goTo(0)

    expect(store.makeMove('e2', 'e5')).toBe(false)
    expect(selectActiveVariation(store.getSnapshot())).toBeNull()
  })
})
