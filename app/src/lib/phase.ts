import type { Phase } from '../types'

export function phaseBoundaries(phases: Phase[]): {
  openingEnd: number
  middlegameEnd: number
} {
  let openingEnd = -1
  let middlegameEnd = -1
  for (let i = 0; i < phases.length; i++) {
    if (phases[i] === 'opening') openingEnd = i
    if (phases[i] === 'opening' || phases[i] === 'middlegame') middlegameEnd = i
  }
  if (phases.length > 0) {
    openingEnd = Math.max(0, openingEnd)
    middlegameEnd = Math.max(0, middlegameEnd)
  }
  return { openingEnd, middlegameEnd }
}
