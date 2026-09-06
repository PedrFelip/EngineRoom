import type { DrawShape } from 'chessground/draw'
import type { Key } from 'chessground/types'

export interface BoardArrow {
  from: string
  to: string
  brush?: string
}

/** Setas da engine usam a camada automática, imune à limpeza por clique. */
export function analysisDrawing(arrows: BoardArrow[]) {
  return {
    autoShapes: arrows.map(
      (arrow): DrawShape => ({
        orig: arrow.from as Key,
        dest: arrow.to as Key,
        brush: arrow.brush ?? 'green',
      }),
    ),
  }
}
