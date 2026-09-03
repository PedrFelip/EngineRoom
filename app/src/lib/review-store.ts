/**
 * Máquina de estados pura da revisão: cursor da linha principal. Zero I/O e
 * zero React — as transições (navegação) vivem aqui e o hook `useReview` é só
 * a ponte de view. O snapshot é imutável e referencialmente estável entre
 * transições (pronto para `useSyncExternalStore`).
 */

import { Chess } from 'chess.js'
import type { ReviewResult } from '../types'

export interface VariationMove {
  id: string
  uci: string
  san: string
  fen: string
  children: VariationMove[]
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
}

export interface ReviewStore {
  getSnapshot(): ReviewStoreSnapshot
  subscribe(listener: () => void): () => void
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
}

export function createReviewStore(): ReviewStore {
  let nextVariationId = 1
  let snapshot: ReviewStoreSnapshot = {
    result: null,
    currentPly: 0,
    variation: null,
    variations: [],
  }
  const listeners = new Set<() => void>()

  function commit(next: Partial<ReviewStoreSnapshot>): void {
    const nextSnapshot = { ...snapshot, ...next }
    if (
      nextSnapshot.result === snapshot.result &&
      nextSnapshot.currentPly === snapshot.currentPly &&
      nextSnapshot.variation === snapshot.variation &&
      nextSnapshot.variations === snapshot.variations
    ) {
      return
    }
    snapshot = nextSnapshot
    for (const l of listeners) l()
  }

  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    setResult(result: ReviewResult) {
      commit({
        result,
        currentPly: result.moves.length,
        variation: null,
        variations: [],
      })
    },
    goTo(ply: number) {
      const total = snapshot.result?.moves.length ?? 0
      commit({
        currentPly: Math.max(0, Math.min(total, ply)),
        variation: null,
      })
    },
    next() {
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
      commit({
        currentPly: snapshot.result?.moves.length ?? 0,
        variation: null,
      })
    },
    makeMove(from, to, promotion = 'q') {
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
          commit({
            variation: {
              ...variation,
              path: [...variation.path, existing.id],
            },
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
      const result = snapshot.result
      const fen = result?.positions[snapshot.currentPly]?.fen
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
  }
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
