import type { AnalysisKind } from '../types'

export type AdaptiveProfileId = 'fast' | 'deep'

export interface AdaptiveProfile {
  id: AdaptiveProfileId
  label: string
  /** Busca ampla: curta, mas já com alternativas para estimar a decisão. */
  triageMs: number
  triageMultipv: number
  mediumMs: number
  highMs: number
  refinementMultipv: number
  /** Limite das posições refinadas; gatilhos duros não são descartados. */
  maxRefineFraction: number
  minRefinePositions: number
}

export const ADAPTIVE_PROFILES: Record<AdaptiveProfileId, AdaptiveProfile> = {
  fast: {
    id: 'fast',
    label: 'Automático rápido',
    triageMs: 120,
    triageMultipv: 2,
    mediumMs: 600,
    highMs: 1_500,
    refinementMultipv: 2,
    maxRefineFraction: 0.2,
    minRefinePositions: 6,
  },
  deep: {
    id: 'deep',
    label: 'Automático profundo',
    triageMs: 300,
    triageMultipv: 3,
    mediumMs: 1_500,
    highMs: 4_000,
    refinementMultipv: 3,
    maxRefineFraction: 0.35,
    minRefinePositions: 10,
  },
}

export function adaptiveProfileForKind(
  kind: AnalysisKind | undefined,
): AdaptiveProfile | null {
  if (kind === 'auto-fast') return ADAPTIVE_PROFILES.fast
  if (kind === 'auto-deep') return ADAPTIVE_PROFILES.deep
  return null
}
