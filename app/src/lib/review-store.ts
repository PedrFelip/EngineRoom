/**
 * Máquina de estados pura da revisão: cursor da linha principal. Zero I/O e
 * zero React — as transições (navegação) vivem aqui e o hook `useReview` é só
 * a ponte de view. O snapshot é imutável e referencialmente estável entre
 * transições; Zustand vanilla gerencia snapshots e assinaturas.
 */

import { Chess } from 'chess.js'
import { createStore } from 'zustand/vanilla'
import type { Classification, PositionAnalysis, ReviewResult } from '../types'
import { preferredAnalysis } from './analysis-quality'

import {
  appendAtPath,
  childrenAtPath,
  mergeLineAtPath,
  nodeAtPath,
  updateNodeClassification,
  type VariationMove,
} from './variation-tree'

export { nodeAtPath, type VariationMove } from './variation-tree'

export interface LiveAnalysisState {
  fen: string | null
  status: 'idle' | 'running' | 'cancelled' | 'error'
  error: string | null
  positions: Record<string, PositionAnalysis>
}

export interface ReviewVariation {
  id: string
  basePly: number
  roots: VariationMove[]
  path: string[]
}

export interface ReviewStoreSnapshot {
  result: ReviewResult | null
  currentPly: number
  variation: ReviewVariation | null
  variations: ReviewVariation[]
  liveAnalysis: LiveAnalysisState
}

export interface ReviewStore {
  getState(): ReviewStoreSnapshot
  getInitialState(): ReviewStoreSnapshot
  getSnapshot(): ReviewStoreSnapshot
  subscribe(
    listener: (
      state: ReviewStoreSnapshot,
      previous: ReviewStoreSnapshot,
    ) => void,
  ): () => void
  /** Instala o resultado (análise nova ou reabertura) e salta ao último lance. */
  setResult(result: ReviewResult): void
  goTo(ply: number): void
  next(): void
  prev(): void
  first(): void
  last(): void
  makeMove(from: string, to: string, promotion?: string): boolean
  exploreLine(pv: string[]): string[]
  goToVariation(variationId: string, path: string[]): void
  exitVariation(): void
  cancelLiveAnalysis(): void
  startLiveAnalysis(fen: string): void
  setLiveAnalysis(fen: string, analysis: PositionAnalysis): void
  failLiveAnalysis(fen: string, error: string): void
  setVariationClassification(nodeId: string, value: Classification): void
}

export function createReviewStore(): ReviewStore {
  let nextVariationId = 1
  const state = createStore<ReviewStoreSnapshot>(() => ({
    result: null,
    currentPly: 0,
    variation: null,
    variations: [],
    liveAnalysis: {
      fen: null,
      status: 'idle',
      error: null,
      positions: {},
    },
  }))

  function commit(next: Partial<ReviewStoreSnapshot>): void {
    state.setState((snapshot) => {
      const changed = Object.entries(next).some(
        ([key, value]) => snapshot[key as keyof ReviewStoreSnapshot] !== value,
      )
      return changed ? next : snapshot
    })
  }

  function commitVariation(variation: ReviewVariation): void {
    const saved = state.getState().variations
    const exists = saved.some((item) => item.id === variation.id)
    commit({
      currentPly: variation.basePly,
      variation,
      variations: exists
        ? saved.map((item) => (item.id === variation.id ? variation : item))
        : [...saved, variation],
    })
  }

  return {
    getState: state.getState,
    getInitialState: state.getInitialState,
    getSnapshot: state.getState,
    subscribe: state.subscribe,
    setResult(result: ReviewResult) {
      commit({
        result,
        currentPly: result.moves.length,
        variation: null,
        variations: [],
      })
    },
    goTo(ply: number) {
      const snapshot = state.getState()
      const total = snapshot.result?.moves.length ?? 0
      commit({
        currentPly: Math.max(0, Math.min(total, ply)),
        variation: null,
      })
    },
    next() {
      const snapshot = state.getState()
      const variation = snapshot.variation
      if (variation) {
        const next = childrenAtPath(variation.roots, variation.path)[0]
        if (!next) return
        const nextVariation = {
          ...variation,
          path: [...variation.path, next.id],
        }
        commitVariation(nextVariation)
        return
      }
      const total = snapshot.result?.moves.length ?? 0
      commit({ currentPly: Math.min(total, snapshot.currentPly + 1) })
    },
    prev() {
      const snapshot = state.getState()
      if (snapshot.variation) {
        if (snapshot.variation.path.length === 0) {
          commit({ variation: null })
        } else {
          const nextVariation = {
            ...snapshot.variation,
            path: snapshot.variation.path.slice(0, -1),
          }
          commitVariation(nextVariation)
        }
        return
      }
      commit({ currentPly: Math.max(0, snapshot.currentPly - 1) })
    },
    first() {
      commit({ currentPly: 0, variation: null })
    },
    last() {
      const snapshot = state.getState()
      commit({
        currentPly: snapshot.result?.moves.length ?? 0,
        variation: null,
      })
    },
    makeMove(from, to, promotion = 'q') {
      const snapshot = state.getState()
      const result = snapshot.result
      if (!result) return false
      const variation = snapshot.variation
      const basePly = variation?.basePly ?? snapshot.currentPly
      const current = variation
        ? nodeAtPath(variation.roots, variation.path)
        : null
      const fen = current?.fen ?? result.positions[basePly]?.fen
      if (!fen) return false
      try {
        const chess = new Chess(fen)
        const move = chess.move({ from, to, promotion })
        if (!move) return false
        const path = variation?.path ?? []
        const uci = `${move.from}${move.to}${move.promotion ?? ''}`
        const existing = variation
          ? childrenAtPath(variation.roots, path).find(
              (candidate) => candidate.uci === uci,
            )
          : undefined
        if (existing && variation) {
          commitVariation({
            ...variation,
            path: [...path, existing.id],
          })
          return true
        }
        const nextMove: VariationMove = {
          id: `variation-${nextVariationId++}`,
          uci,
          san: move.san,
          fen: chess.fen(),
          children: [],
        }
        commitVariation({
          id: variation?.id ?? `tree-${nextVariationId++}`,
          basePly,
          roots: variation
            ? appendAtPath(variation.roots, path, nextMove)
            : [nextMove],
          path: [...path, nextMove.id],
        })
        return true
      } catch {
        return false
      }
    },
    exploreLine(pv) {
      const snapshot = state.getState()
      const result = snapshot.result
      const active = snapshot.variation
      const basePly = active?.basePly ?? snapshot.currentPly
      const activeNode = active ? nodeAtPath(active.roots, active.path) : null
      const fen = activeNode?.fen ?? result?.positions[basePly]?.fen
      if (!result || !fen || pv.length === 0) return []
      // Valide a PV inteira antes de publicar qualquer mudança na árvore.
      let root: VariationMove | null = null
      let tail: VariationMove | null = null
      const moves: string[] = []
      try {
        const chess = new Chess(fen)
        for (const uci of pv) {
          const move = chess.move({
            from: uci.slice(0, 2),
            to: uci.slice(2, 4),
            promotion: uci[4] ?? 'q',
          })
          const node: VariationMove = {
            id: `variation-${nextVariationId++}`,
            uci: `${move.from}${move.to}${move.promotion ?? ''}`,
            san: move.san,
            fen: chess.fen(),
            children: [],
          }
          moves.push(node.uci)
          if (tail) tail.children.push(node)
          else root = node
          tail = node
        }
      } catch {
        return []
      }
      if (!root) return []
      const saved =
        active ??
        snapshot.variations.find(
          (item) =>
            item.basePly === basePly &&
            item.roots.some((node) => node.uci === moves[0]),
        )
      const path = active?.path ?? []
      const roots = mergeLineAtPath(saved?.roots ?? [], path, root)
      const fullPath = [...path]
      for (const uci of moves) {
        const node = childrenAtPath(roots, fullPath).find(
          (item) => item.uci === uci,
        )
        if (!node) return []
        fullPath.push(node.id)
      }
      commitVariation({
        id: saved?.id ?? `tree-${nextVariationId++}`,
        basePly,
        roots,
        path: fullPath.slice(0, path.length + 1),
      })
      return fullPath
    },
    goToVariation(variationId, path) {
      const snapshot = state.getState()
      const variation = snapshot.variations.find(
        (saved) => saved.id === variationId,
      )
      if (!variation) return
      if (path.length > 0 && !nodeAtPath(variation.roots, path)) return
      const nextVariation = { ...variation, path }
      commitVariation(nextVariation)
    },
    exitVariation() {
      commit({ variation: null })
    },
    cancelLiveAnalysis() {
      const liveAnalysis = state.getState().liveAnalysis
      if (liveAnalysis.status !== 'running') return
      commit({
        liveAnalysis: { ...liveAnalysis, status: 'cancelled', error: null },
      })
    },
    startLiveAnalysis(fen) {
      const snapshot = state.getState()
      commit({
        liveAnalysis: {
          ...snapshot.liveAnalysis,
          fen,
          status: 'running',
          error: null,
        },
      })
    },
    setLiveAnalysis(fen, analysis) {
      const snapshot = state.getState()
      commit({
        liveAnalysis: {
          fen,
          status: 'idle',
          error: null,
          positions: {
            ...snapshot.liveAnalysis.positions,
            [fen]: preferredAnalysis(
              snapshot.liveAnalysis.positions[fen],
              analysis,
            ),
          },
        },
      })
    },
    failLiveAnalysis(fen, error) {
      const snapshot = state.getState()
      if (snapshot.liveAnalysis.fen !== fen) return
      commit({
        liveAnalysis: {
          ...snapshot.liveAnalysis,
          status: 'error',
          error,
        },
      })
    },
    setVariationClassification(nodeId, value) {
      const snapshot = state.getState()
      const variations = snapshot.variations.map((variation) => ({
        ...variation,
        roots: updateNodeClassification(variation.roots, nodeId, value),
      }))
      const variation = snapshot.variation
        ? (variations.find((item) => item.id === snapshot.variation?.id) ??
          null)
        : null
      commit({ variations, variation })
    },
  }
}
