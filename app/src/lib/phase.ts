import type { Phase } from '../types'

export type { Phase }

/**
 * Fases do jogo (Abertura / Meio-jogo / Final) combinando dois sinais:
 *
 *  - Número do lance (ply): define a Abertura — os primeiros lances completos
 *    são Abertura independente do material (a maioria das aberturas chega ao
 *    meio-jogo com material cheio, então material sozinho é um sinal ruim aqui).
 *  - Material não-peão na escala Reinfeld (N=3, B=3, T=5, D=9; peões excluídos):
 *    define o Final — pouco material (<= 24, ~1/3 do máximo inicial 62).
 *
 * Regra (com prioridade):
 *  - Final: mat <= 24                    (material baixo, independente do lance)
 *  - Abertura: ply <= OPENING_MAX_PLY    (primeiros 10 lances completos)
 *  - Meio-jogo: o que estiver entre os dois.
 *
 * `phaseOfMaterial` (só material) fica como fallback p/ posições isoladas sem
 * contexto de lance (variações). Núcleo puro, sem efeitos colaterais.
 */

/** Limiar inclusivo: ply <= este valor é Abertura (10 lances completos). */
const OPENING_MAX_PLY = 12
/** Limiar inclusivo: material >= este valor é Abertura (sinal de material p/ variações). */
const OPENING_MIN = 50
/** Limiar inclusivo: material <= este valor é Final. */
const ENDGAME_MAX = 12

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
 * Fase de cada posição (paralelo ao vetor de entrada), combinando número do
 * lance e material não-peão via `phaseOf`. A fase só avança: uma vez atingida
 * uma fase, posições posteriores com material maior (p.ex. por promoção de peão
 * em dama) não regrediram — garante faixas contíguas no gráfico.
 */
export function computePhases(
  positions: { fen: string; ply: number }[],
): Phase[] {
  let current: Phase = 'opening'
  return positions.map((p) => {
    const raw = phaseOf(p.ply, nonPawnMaterial(p.fen))
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
  return { openingEnd, middlegameEnd }
}

/**
 * Fase combinando material não-peão e número do lance. O Final (material baixo)
 * tem prioridade sobre a Abertura (número do lance), para que trocas massivas
 * precoces já caracterizem o final.
 */
export function phaseOf(ply: number, mat: number): Phase {
  if (mat <= ENDGAME_MAX) return 'endgame'
  if (ply <= OPENING_MAX_PLY) return 'opening'
  return 'middlegame'
}

/**
 * Mapeia um total de material não-peão à fase correspondente (só material).
 * Usado para posições isoladas sem contexto de lance — p.ex. uma casa de uma
 * variação explorada, onde o ply absoluto da partida não está disponível.
 * Limiares inclusivos nas bordas (50 é Abertura, 24 é Final).
 */
export function phaseOfMaterial(mat: number): Phase {
  if (mat >= OPENING_MIN) return 'opening'
  if (mat <= ENDGAME_MAX) return 'endgame'
  return 'middlegame'
}
