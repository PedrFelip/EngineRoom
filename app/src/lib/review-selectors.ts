import { nodeAtPath, type ReviewStoreSnapshot } from './review-store'

export function selectVariationMove(state: ReviewStoreSnapshot) {
  const variation = state.variation
  return variation ? nodeAtPath(variation.roots, variation.path) : null
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
  const live = fen ? state.liveAnalysis.positions[fen] : undefined
  if (live) return live
  if (state.variation) return null
  return state.result?.positions[state.currentPly] ?? null
}

export function selectSourceFen(state: ReviewStoreSnapshot) {
  const variation = state.variation
  if (!variation) return undefined
  const parent = nodeAtPath(variation.roots, variation.path.slice(0, -1))
  return parent?.fen ?? state.result?.positions[variation.basePly]?.fen
}

export function selectSourcePosition(state: ReviewStoreSnapshot) {
  const fen = selectSourceFen(state)
  if (!fen || !state.variation) return undefined
  const live = state.liveAnalysis.positions[fen]
  if (live?.fen === fen) return live
  const base = state.result?.positions[state.variation.basePly]
  return base?.fen === fen ? base : undefined
}
