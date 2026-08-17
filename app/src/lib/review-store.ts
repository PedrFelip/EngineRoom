/**
 * Máquina de estados pura da revisão: cursor da linha principal + árvore de
 * variações jogadas pelo usuário. Zero I/O e zero React — as transições
 * (navegação, `makeMove`, refino ao vivo) vivem aqui e o hook `useReview` é
 * só a ponte de view. O snapshot é imutável e referencialmente estável entre
 * transições (pronto para `useSyncExternalStore`).
 */

import { Chess } from 'chess.js'
import type {
  ReviewResult,
  Variation,
  VariationMap,
  VariationMove,
} from '../types'
import type { RawPosition } from './analyze'
import {
  applyLiveToVariation,
  decideUserMove,
  defaultBeforeCpResolver,
} from './variations'

/** Foco numa variação: qual variação, ramificada de qual ply, em qual lance. */
export interface ReviewFocus {
  variationId: string
  parentPly: number
  ply: number
}

/** Alvo do refino ao vivo: posição da linha principal ou de um lance de variação. */
export type AnalysisTarget =
  | { kind: 'mainline' }
  | { kind: 'variation'; variationId: string; moveId: string }

export interface ReviewStoreSnapshot {
  result: ReviewResult | null
  currentPly: number
  variations: VariationMap
  currentVariation: ReviewFocus | null
}

export interface ReviewStore {
  getSnapshot(): ReviewStoreSnapshot
  subscribe(listener: () => void): () => void
  /** FEN exibido: o da variação em foco, senão o da linha principal. */
  getDisplayedFen(): string | null
  /** Alvo que o refino ao vivo deve avaliar (derivado do foco atual). */
  getAnalysisTarget(): AnalysisTarget
  /** Instala o resultado (análise nova ou reabertura) e salta ao último lance. */
  setResult(result: ReviewResult): void
  goTo(ply: number): void
  next(): void
  prev(): void
  first(): void
  last(): void
  /** Navega para um lance de uma variação (currentPly vira o ply-pai). */
  goToVariation(variationId: string, parentPly: number, ply: number): void
  /** Abandona a variação e volta para a linha principal no ply-pai. */
  exitVariation(): void
  /** Aplica um lance arrastado pelo usuário: avança a linha ou abre/acrescenta variação. */
  makeMove(uci: string): void
  applyLive(
    target: { variationId: string; moveId: string },
    raw: RawPosition,
  ): void
}

export function createReviewStore(): ReviewStore {
  let snapshot: ReviewStoreSnapshot = {
    result: null,
    currentPly: 0,
    variations: {},
    currentVariation: null,
  }
  const listeners = new Set<() => void>()
  let idCounter = 0
  const nextId = (prefix: 'm' | 'v') => {
    idCounter += 1
    return `${prefix}${idCounter}`
  }

  function commit(next: Partial<ReviewStoreSnapshot>): void {
    snapshot = { ...snapshot, ...next }
    for (const l of listeners) l()
  }

  function findFocus(): Variation | null {
    const focus = snapshot.currentVariation
    if (!focus) return null
    const list = snapshot.variations[focus.parentPly] ?? []
    return list.find((v) => v.id === focus.variationId) ?? null
  }

  function getDisplayedFen(): string | null {
    const focus = snapshot.currentVariation
    if (focus) {
      const v = findFocus()
      const move = v?.moves[focus.ply - 1]
      if (move) return move.fenAfter
    }
    return snapshot.result?.positions[snapshot.currentPly]?.fen ?? null
  }

  /**
   * Valida um lance UCI contra a posição exibida via chess.js puro (instância
   * descartável) e devolve os campos do lance, ou null se ilegal.
   */
  function validateMove(uci: string): {
    color: 'w' | 'b'
    san: string
    fenBefore: string
    fenAfter: string
  } | null {
    const fen = getDisplayedFen()
    if (!fen) return null
    try {
      const chess = new Chess(fen)
      const move = chess.move({
        from: uci.slice(0, 2),
        to: uci.slice(2, 4),
        promotion: uci.length > 4 ? uci[4] : undefined,
      })
      return {
        color: move.color,
        san: move.san,
        fenBefore: fen,
        fenAfter: chess.fen(),
      }
    } catch {
      return null
    }
  }

  function makeMove(uci: string): void {
    const { result, currentVariation, variations } = snapshot
    if (!result) return
    const played = validateMove(uci)
    if (!played) return

    // Dentro de variação: acrescenta à variação em foco.
    if (currentVariation) {
      const list = variations[currentVariation.parentPly] ?? []
      const vIdx = list.findIndex((v) => v.id === currentVariation.variationId)
      if (vIdx === -1) return
      const v = list[vIdx]
      const newMove: VariationMove = {
        id: nextId('m'),
        ply: v.moves.length + 1,
        color: played.color,
        san: played.san,
        uci,
        fenBefore: played.fenBefore,
        fenAfter: played.fenAfter,
      }
      const newList = [
        ...list.slice(0, vIdx),
        { ...v, moves: [...v.moves, newMove] },
        ...list.slice(vIdx + 1),
      ]
      commit({
        variations: { ...variations, [v.parentPly]: newList },
        currentVariation: {
          variationId: v.id,
          parentPly: v.parentPly,
          ply: newMove.ply,
        },
      })
      return
    }

    // Na linha principal: avança se coincide com o próximo lance, senão ramifica.
    const nextUci = result.moves[snapshot.currentPly]?.uci ?? null
    const decision = decideUserMove(uci, snapshot.currentPly, nextUci)
    if (decision.kind === 'advance') {
      commit({ currentPly: snapshot.currentPly + 1, currentVariation: null })
      return
    }
    const parentPly = decision.parentPly
    const firstMove: VariationMove = {
      id: nextId('m'),
      ply: 1,
      color: played.color,
      san: played.san,
      uci,
      fenBefore: played.fenBefore,
      fenAfter: played.fenAfter,
    }
    const newV: Variation = {
      id: nextId('v'),
      parentPly,
      moves: [firstMove],
    }
    const list = variations[parentPly] ?? []
    commit({
      variations: { ...variations, [parentPly]: [...list, newV] },
      currentVariation: { variationId: newV.id, parentPly, ply: 1 },
    })
  }

  function getAnalysisTarget(): AnalysisTarget {
    const focus = snapshot.currentVariation
    if (focus) {
      const v = findFocus()
      const move = v?.moves[focus.ply - 1]
      if (move) {
        return {
          kind: 'variation',
          variationId: focus.variationId,
          moveId: move.id,
        }
      }
    }
    return { kind: 'mainline' }
  }

  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    getDisplayedFen,
    getAnalysisTarget,
    setResult(result: ReviewResult) {
      commit({ result, currentPly: result.moves.length })
    },
    goTo(ply: number) {
      const total = snapshot.result?.moves.length ?? 0
      commit({
        currentPly: Math.max(0, Math.min(total, ply)),
        currentVariation: null,
      })
    },
    next() {
      const focus = snapshot.currentVariation
      if (focus) {
        // Dentro de variação: avança o foco até o fim dela e para.
        const v = findFocus()
        const len = v?.moves.length ?? 0
        if (focus.ply < len) {
          commit({ currentVariation: { ...focus, ply: focus.ply + 1 } })
        }
        return
      }
      const total = snapshot.result?.moves.length ?? 0
      commit({ currentPly: Math.min(total, snapshot.currentPly + 1) })
    },
    prev() {
      const focus = snapshot.currentVariation
      if (focus) {
        // No primeiro lance da variação: sai para a linha principal no ply-pai.
        if (focus.ply > 1) {
          commit({ currentVariation: { ...focus, ply: focus.ply - 1 } })
        } else {
          commit({ currentVariation: null })
        }
        return
      }
      commit({ currentPly: Math.max(0, snapshot.currentPly - 1) })
    },
    first() {
      commit({ currentPly: 0, currentVariation: null })
    },
    last() {
      commit({
        currentPly: snapshot.result?.moves.length ?? 0,
        currentVariation: null,
      })
    },
    goToVariation(variationId: string, parentPly: number, ply: number) {
      // Invariante: dentro de variação, currentPly === parentPly → sair
      // (exitVariation/prev no ply 1) volta ao ponto de ramificação.
      commit({
        currentPly: parentPly,
        currentVariation: { variationId, parentPly, ply },
      })
    },
    exitVariation() {
      commit({ currentVariation: null })
    },
    makeMove,
    applyLive(
      target: { variationId: string; moveId: string },
      raw: RawPosition,
    ) {
      // O cp "antes" do lance vem da linha principal (ply 1 da variação) ou
      // do afterCp do lance anterior — o mesmo modelo do buildReview.
      const resolveBeforeCp = (v: Variation, m: VariationMove) =>
        defaultBeforeCpResolver(
          v,
          m,
          (ply) => snapshot.result?.positions[ply]?.cp,
        )
      const next = applyLiveToVariation(
        snapshot.variations,
        target,
        raw,
        resolveBeforeCp,
      )
      // Mesma ref = alvo inexistente/beforeCp pendente: lance segue pendente e
      // ninguém é notificado (equivale ao bail-out do setState com ref igual).
      if (next === snapshot.variations) return
      commit({ variations: next })
    },
  }
}
