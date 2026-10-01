import type { AnalysisKind } from '../types'

export type AdaptiveProfileId = 'fast' | 'deep'

export interface AdaptiveProfile {
  id: AdaptiveProfileId
  label: string
  /** Linhas candidatas de todas as posições, antes dos refinamentos. */
  triageMs: number
  triageMultipv: number
  mediumMs: number
  highMs: number
  refinementMultipv: number
  mediumMultipv: number
  /** Limite das posições refinadas; gatilhos duros não são descartados. */
  maxRefineFraction: number
  minRefinePositions: number
}

export const ADAPTIVE_PROFILES: Record<AdaptiveProfileId, AdaptiveProfile> = {
  fast: {
    id: 'fast',
    label: 'Automático rápido',
    triageMs: 180,
    triageMultipv: 3,
    mediumMs: 500,
    highMs: 2_000,
    refinementMultipv: 2,
    mediumMultipv: 1,
    maxRefineFraction: 0.15,
    minRefinePositions: 4,
  },
  deep: {
    id: 'deep',
    label: 'Automático profundo',
    triageMs: 450,
    triageMultipv: 5,
    mediumMs: 1_800,
    highMs: 6_000,
    refinementMultipv: 2,
    mediumMultipv: 1,
    maxRefineFraction: 0.25,
    minRefinePositions: 6,
  },
}

export function adaptiveProfileForKind(
  kind: AnalysisKind | undefined,
): AdaptiveProfile | null {
  if (kind === 'auto-fast') return ADAPTIVE_PROFILES.fast
  if (kind === 'auto-deep') return ADAPTIVE_PROFILES.deep
  return null
}
