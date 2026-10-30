import { preferredAnalysis } from './analysis-quality'
import {
  nodeAtPath,
  type ReviewStoreSnapshot,
  selectActiveVariation,
} from './review-store'

export function selectVariationMove(state: ReviewStoreSnapshot) {
  const variation = selectActiveVariation(state)
  return variation ? nodeAtPath(variation.roots, variation.path) : null
}

export function selectDisplayedPly(state: ReviewStoreSnapshot) {
  const variation = selectActiveVariation(state)
  return variation
    ? variation.basePly + variation.path.length
    : state.currentPly
}

export function selectDisplayedFen(state: ReviewStoreSnapshot) {
  return (
    selectVariationMove(state)?.fen ??
    state.result?.positions[state.currentPly]?.fen ??
    null
  )
}

export function selectDisplayedPosition(state: ReviewStoreSnapshot) {
  const fen = selectDisplayedFen(state)
  if (!fen) return null
  // Rust publishes complete mainline positions in ply order, including ply zero.
  const base = state.result?.positions[state.currentPly]
  const original = base?.fen === fen ? base : undefined
  const live = state.liveAnalysis.positions[fen]
  if (live?.fen === fen) return preferredAnalysis(original, live)
  return original ?? null
}

export function selectSourceFen(state: ReviewStoreSnapshot) {
  const variation = selectActiveVariation(state)
  if (!variation) return undefined
  const parent = nodeAtPath(variation.roots, variation.path.slice(0, -1))
  return parent?.fen ?? state.result?.positions[variation.basePly]?.fen
}

export function selectSourcePosition(state: ReviewStoreSnapshot) {
  const fen = selectSourceFen(state)
  const variation = selectActiveVariation(state)
  if (!fen || !variation) return undefined
  const live = state.liveAnalysis.positions[fen]
  const base = state.result?.positions[variation.basePly]
  const original = base?.fen === fen ? base : undefined
  if (live?.fen === fen) return preferredAnalysis(original, live)
  return original
}
