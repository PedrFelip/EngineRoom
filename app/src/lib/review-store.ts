/**
 * Máquina de estados pura da revisão: cursor da linha principal. Zero I/O e
 * zero React — as transições (navegação) vivem aqui e o hook `useReview` é só
 * a ponte de view. O snapshot é imutável e referencialmente estável entre
 * transições (pronto para `useSyncExternalStore`).
 */

import type { ReviewResult } from '../types'

export interface ReviewStoreSnapshot {
  result: ReviewResult | null
  currentPly: number
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
}

export function createReviewStore(): ReviewStore {
  let snapshot: ReviewStoreSnapshot = {
    result: null,
    currentPly: 0,
  }
  const listeners = new Set<() => void>()

  function commit(next: Partial<ReviewStoreSnapshot>): void {
    const nextSnapshot = { ...snapshot, ...next }
    if (
      nextSnapshot.result === snapshot.result &&
      nextSnapshot.currentPly === snapshot.currentPly
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
      commit({ result, currentPly: result.moves.length })
    },
    goTo(ply: number) {
      const total = snapshot.result?.moves.length ?? 0
      commit({
        currentPly: Math.max(0, Math.min(total, ply)),
      })
    },
    next() {
      const total = snapshot.result?.moves.length ?? 0
      commit({ currentPly: Math.min(total, snapshot.currentPly + 1) })
    },
    prev() {
      commit({ currentPly: Math.max(0, snapshot.currentPly - 1) })
    },
    first() {
      commit({ currentPly: 0 })
    },
    last() {
      commit({
        currentPly: snapshot.result?.moves.length ?? 0,
      })
    },
  }
}
