import { ENGINE_TIERS, type EngineMode } from '../types'

export interface EngineTagInput {
  mode: EngineMode
  /** Profundidade (mode='depth') ou milissegundos (mode='time'). */
  depth: number
  engineTier: string
}

/**
 * Rótulo curto do modo de análise usado na revisão, para listas e cabeçalhos.
 *  - Profundidade: "Equilibrado · d20" (tier label + depth).
 *  - Tempo: "Tempo · 5s/lance" (milissegundos → segundos).
 */
export function formatEngineTag({
  mode,
  depth,
  engineTier,
}: EngineTagInput): string {
  if (mode === 'time') {
    const seconds = Math.max(1, Math.round(depth / 1000))
    return `Tempo · ${seconds}s/lance`
  }
  const tier = ENGINE_TIERS.find((t) => t.id === engineTier)
  const label = tier?.label ?? `d${depth}`
  return `${label} · d${depth}`
}
