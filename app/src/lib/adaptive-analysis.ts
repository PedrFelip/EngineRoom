import profiles from '../../src-tauri/src/review/profiles.json'
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
  contextPlies: number
}

// Profile budgets come from the same JSON embedded in the Rust analysis core.
export const ADAPTIVE_PROFILES: Record<AdaptiveProfileId, AdaptiveProfile> = {
  fast: { id: 'fast', label: 'Automático rápido', ...profiles.fast },
  deep: { id: 'deep', label: 'Automático profundo', ...profiles.deep },
}

export function adaptiveProfileForKind(
  kind: AnalysisKind | undefined,
): AdaptiveProfile | null {
  if (kind === 'auto-fast') return ADAPTIVE_PROFILES.fast
  if (kind === 'auto-deep') return ADAPTIVE_PROFILES.deep
  return null
}
