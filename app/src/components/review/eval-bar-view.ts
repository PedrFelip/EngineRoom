import { evalLabel } from '../../lib/eval-label'
import type { PositionAnalysis } from '../../types'

/** Placeholder used elsewhere in the review UI when a value is unavailable. */
export const EVAL_UNAVAILABLE_LABEL = '—'

export interface EvalBarView {
  winPct: number
  label: string | undefined
}

/**
 * Deriva a barra apenas da posição exibida. Nunca reaproveita a avaliação de
 * outra posição: variações ainda sem busca ao vivo mostram indisponível.
 */
export function evalBarView(
  position: PositionAnalysis | null | undefined,
  analyzed: boolean,
): EvalBarView {
  if (!analyzed) return { winPct: 50, label: undefined }
  if (!position) return { winPct: 50, label: EVAL_UNAVAILABLE_LABEL }
  const stm = position.fen.split(' ')[1] === 'b' ? 'b' : 'w'
  return {
    winPct: position.winPct,
    label: evalLabel(position.cp, position.fen, stm),
  }
}
