import { clear } from 'chessground/draw'
import type { State } from 'chessground/state'
import { describe, expect, it, vi } from 'vitest'
import { analysisDrawing } from './board-drawing'

describe('setas da análise e limpeza por clique do Chessground', () => {
  it('limpa desenhos manuais sem apagar as setas da engine', () => {
    const drawing = analysisDrawing([{ from: 'e2', to: 'e4', brush: 'blue' }])
    // A rotina chamada pelo clique em casa vazia só precisa destes campos.
    const state = {
      drawable: {
        ...drawing,
        shapes: [{ orig: 'd2', dest: 'd4', brush: 'green' }],
      },
      dom: { redraw: vi.fn() },
    } as unknown as State
    clear(state)
    expect(state.drawable.shapes).toEqual([])
    expect(state.drawable.autoShapes).toEqual([
      { orig: 'e2', dest: 'e4', brush: 'blue' },
    ])
    clear(state)
    expect(state.drawable.autoShapes).toHaveLength(1)
  })

  it('não mistura setas de posições anteriores nem mantém setas quando não há análise', () => {
    const first = analysisDrawing([{ from: 'e2', to: 'e4' }])
    const next = analysisDrawing([{ from: 'e7', to: 'e5' }])
    expect(next.autoShapes).toEqual([
      { orig: 'e7', dest: 'e5', brush: 'green' },
    ])
    expect(first.autoShapes[0].orig).toBe('e2')
    expect(analysisDrawing([]).autoShapes).toEqual([])
  })
})
