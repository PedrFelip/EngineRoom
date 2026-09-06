/**
 * Máquina de estados pura da revisão: cursor da linha principal. Zero I/O e
 * zero React — as transições (navegação) vivem aqui e o hook `useReview` é só
 * a ponte de view. O snapshot é imutável e referencialmente estável entre
 * transições; Zustand vanilla gerencia snapshots e assinaturas.
 */

import { Chess } from 'chess.js'
import { createStore } from 'zustand/vanilla'
import type { Classification, PositionAnalysis, ReviewResult } from '../types'

export interface VariationMove {
  id: string
  uci: string
  san: string
  fen: string
  classification?: Classification
  children: VariationMove[]
}

export interface LiveAnalysisState {
  fen: string | null
  status: 'idle' | 'running' | 'error'
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
  exploreLine(pv: string[]): void
  goToVariation(variationId: string, path: string[]): void
  exitVariation(): void
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
        const current = nodeAtPath(variation.roots, variation.path)
        const next =
          variation.path.length === 0
            ? variation.roots[0]
            : current?.children[0]
        if (!next) return
        const nextVariation = {
          ...variation,
          path: [...variation.path, next.id],
        }
        commit({
          variation: nextVariation,
          variations: snapshot.variations.map((saved) =>
            saved.id === nextVariation.id ? nextVariation : saved,
          ),
        })
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
          commit({
            variation: nextVariation,
            variations: snapshot.variations.map((saved) =>
              saved.id === nextVariation.id ? nextVariation : saved,
            ),
          })
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
        const nextMove = {
          id: `variation-${nextVariationId++}`,
          uci: `${move.from}${move.to}${move.promotion ?? ''}`,
          san: move.san,
          fen: chess.fen(),
          children: [],
        }
        const existing = current?.children.find(
          (candidate) => candidate.uci === nextMove.uci,
        )
        if (existing && variation) {
          const nextVariation = {
            ...variation,
            path: [...variation.path, existing.id],
          }
          commit({
            variation: nextVariation,
            variations: snapshot.variations.map((saved) =>
              saved.id === variation.id ? nextVariation : saved,
            ),
          })
          return true
        }
        const path = variation?.path ?? []
        const roots = variation
          ? appendAtPath(variation.roots, path, nextMove)
          : [nextMove]
        const nextVariation: ReviewVariation = {
          id: variation?.id ?? `tree-${nextVariationId++}`,
          basePly,
          roots,
          path: [...path, nextMove.id],
        }
        const variations = variation
          ? snapshot.variations.map((saved) =>
              saved.id === variation.id ? nextVariation : saved,
            )
          : [...snapshot.variations, nextVariation]
        commit({ variation: nextVariation, variations })
        return true
      } catch {
        return false
      }
    },
    exploreLine(pv) {
      const snapshot = state.getState()
      const result = snapshot.result
      const activeVariation = snapshot.variation
      const activeNode = activeVariation
        ? nodeAtPath(activeVariation.roots, activeVariation.path)
        : null
      const fen =
        activeNode?.fen ?? result?.positions[snapshot.currentPly]?.fen ?? null
      if (!result || !fen || pv.length === 0) return
      const chess = new Chess(fen)
      let root: VariationMove | null = null
      let tail: VariationMove | null = null
      const path: string[] = []
      try {
        for (const uci of pv) {
          const move = chess.move({
            from: uci.slice(0, 2),
            to: uci.slice(2, 4),
            promotion: uci[4] ?? 'q',
          })
          if (!move) break
          const node: VariationMove = {
            id: `variation-${nextVariationId++}`,
            uci,
            san: move.san,
            fen: chess.fen(),
            children: [],
          }
          path.push(node.id)
          if (tail) tail.children.push(node)
          else root = node
          tail = node
        }
      } catch {
        return
      }
      if (root) {
        if (activeVariation) {
          const roots = appendAtPath(
            activeVariation.roots,
            activeVariation.path,
            root,
          )
          const nextVariation = {
            ...activeVariation,
            roots,
            path: [...activeVariation.path, root.id],
          }
          commit({
            variation: nextVariation,
            variations: snapshot.variations.map((saved) =>
              saved.id === nextVariation.id ? nextVariation : saved,
            ),
          })
          return
        }
        const nextVariation: ReviewVariation = {
          id: `tree-${nextVariationId++}`,
          basePly: snapshot.currentPly,
          roots: [root],
          path: path.slice(0, 1),
        }
        commit({
          variation: nextVariation,
          variations: [...snapshot.variations, nextVariation],
        })
      }
    },
    goToVariation(variationId, path) {
      const snapshot = state.getState()
      const variation = snapshot.variations.find(
        (saved) => saved.id === variationId,
      )
      if (!variation) return
      if (path.length > 0 && !nodeAtPath(variation.roots, path)) return
      const nextVariation = { ...variation, path }
      commit({
        currentPly: variation.basePly,
        variation: nextVariation,
        variations: snapshot.variations.map((saved) =>
          saved.id === variationId ? nextVariation : saved,
        ),
      })
    },
    exitVariation() {
      commit({ variation: null })
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
          positions: { ...snapshot.liveAnalysis.positions, [fen]: analysis },
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

function updateNodeClassification(
  nodes: VariationMove[],
  nodeId: string,
  value: Classification,
): VariationMove[] {
  let changed = false
  const next = nodes.map((node) => {
    if (node.id === nodeId) {
      changed = true
      return { ...node, classification: value }
    }
    const children = updateNodeClassification(node.children, nodeId, value)
    if (children === node.children) return node
    changed = true
    return { ...node, children }
  })
  return changed ? next : nodes
}

export function nodeAtPath(
  roots: VariationMove[],
  path: string[],
): VariationMove | null {
  let candidates = roots
  let current: VariationMove | null = null
  for (const id of path) {
    current = candidates.find((node) => node.id === id) ?? null
    if (!current) return null
    candidates = current.children
  }
  return current
}

function appendAtPath(
  roots: VariationMove[],
  path: string[],
  move: VariationMove,
): VariationMove[] {
  if (path.length === 0) return [...roots, move]
  const [id, ...rest] = path
  return roots.map((node) =>
    node.id === id
      ? { ...node, children: appendAtPath(node.children, rest, move) }
      : node,
  )
}
