import enginePolicy from '../../src-tauri/src/review/engine-policy.json'
import { ENGINE_TIERS, type EngineTier } from '../types'

export const MIN_DEPTH = enginePolicy.minDepth
export const MAX_DEPTH = enginePolicy.maxDepth

/**
 * Resolve um `EngineTier` para qualquer profundidade entre MIN_DEPTH e
 * MAX_DEPTH. Se a depth bater com a de um preset (15/20/25), devolve o tier
 * canônico; caso contrário, devolve um tier sintético `id='custom'` carregando
 * a depth informada. Valores fora do intervalo são clampados.
 */
export function resolveEngineTier(depth: number): EngineTier {
  const clamped = Math.max(MIN_DEPTH, Math.min(MAX_DEPTH, Math.round(depth)))
  const matched = ENGINE_TIERS.find((t) => t.depth === clamped)
  if (matched) return matched
  return {
    id: 'custom',
    label: enginePolicy.customLabel,
    depth: clamped,
    hint: `${enginePolicy.customHintPrefix}${clamped}${enginePolicy.customHintSuffix}`,
  }
}
