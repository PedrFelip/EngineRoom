import type { Phase } from '../types'

export type { Phase }

/**
 * Fases do jogo (Abertura / Meio-jogo / Final) a partir do material não-peão,
 * contado na escala Reinfeld (N=3, B=3, T=5, D=9). Peões são excluídos —
 * sobrevivem ao final e não discriminam a fase.
 *
 * Limiares sobre o total de material não-peão dos dois lados (máximo inicial = 62):
 *  - Abertura: mat >= 50   (poucas trocas, fase de desenvolvimento)
 *  - Meio-jogo: 24 < mat < 50
 *  - Final: mat <= 24       (~1/3 do material inicial)
 *
 * Núcleo puro, sem efeitos colaterais.
 */

/** Limiar inclusivo: material >= este valor é Abertura. */
const OPENING_MIN = 50
/** Limiar inclusivo: material <= este valor é Final. */
const ENDGAME_MAX = 24

/** Valores Reinfeld das peças não-peão (peões e reis não entram). */
const REINFELD: Record<string, number> = { n: 3, b: 3, r: 5, q: 9 }

/**
 * Conta o material não-peão total (ambos os lados) na escala Reinfeld a partir
 * do FEN. Peões e reis são ignorados. Lê apenas o campo de posicionamento de
 * peças (antes do primeiro espaço) — robusto a FENs degenerados.
 */
export function nonPawnMaterial(fen: string): number {
  const placement = fen.split(' ')[0]
  let total = 0
  for (const ch of placement) {
    const v = REINFELD[ch.toLowerCase()]
    if (v !== undefined) total += v
  }
  return total
}

/** Ordem das fases (Abertura < Meio-jogo < Final) para o travamento monotônico. */
const PHASE_ORDER: Record<Phase, number> = {
  opening: 0,
  middlegame: 1,
  endgame: 2,
}

/**
 * Fase de cada posição (paralelo ao vetor de entrada). A fase só avança: uma
 * vez atingida uma fase, posições posteriores com material maior (p.ex. por
 * promoção de peão em dama) não regrediram — garante faixas contíguas no gráfico.
 */
export function computePhases(positions: { fen: string }[]): Phase[] {
  let current: Phase = 'opening'
  return positions.map((p) => {
    const raw = phaseOfMaterial(nonPawnMaterial(p.fen))
    if (PHASE_ORDER[raw] > PHASE_ORDER[current]) current = raw
    return current
  })
}

/**
 * Índices (ply) onde cada fase termina, para desenhar as faixas do gráfico:
 *  - `openingEnd`: último ply da Abertura (início do Meio-jogo).
 *  - `middlegameEnd`: último ply antes do Final (início do Final).
 * Fases ausentes colapsam: sem Meio-jogo, `middlegameEnd === openingEnd`
 * (faixa do meio com largura 0); sem Final, `middlegameEnd` é o último índice.
 */
export function phaseBoundaries(
  phases: Phase[],
): { openingEnd: number; middlegameEnd: number } {
  let openingEnd = -1
  let middlegameEnd = -1
  for (let i = 0; i < phases.length; i++) {
    if (phases[i] === 'opening') openingEnd = i
    if (phases[i] === 'opening' || phases[i] === 'middlegame')
      middlegameEnd = i
  }
  return { openingEnd, middlegameEnd }
}

/**
 * Mapeia um total de material não-peão à fase correspondente.
 * Limiares inclusivos nas bordas (50 é Abertura, 24 é Final).
 */
export function phaseOfMaterial(mat: number): Phase {
  if (mat >= OPENING_MIN) return 'opening'
  if (mat <= ENDGAME_MAX) return 'endgame'
  return 'middlegame'
}
