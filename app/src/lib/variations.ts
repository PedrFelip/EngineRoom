/**
 * Núcleo puro das linhas alternativas (variações jogadas pelo usuário durante a
 * revisão). Classifica um lance a partir das avaliações (centipawns, POV do lado
 * a jogar) antes e depois dele — espelha a matemática de `buildReview` para que
 * a nota de uma jogada explorada use o mesmo modelo da linha principal.
 */

import type {
  Classification,
  PvLine,
  Variation,
  VariationMap,
  VariationMove,
} from '../types'
import type { RawLine, RawPosition } from './analyze'
import { classifyMove, cpToWinPct } from './scoring'

export interface VariationMoveJudgement {
  winPctBefore: number
  winPctAfter: number
  winPctLoss: number
  classification: Classification
}

/**
 * Classifica um lance de variação a partir do cp (POV do lado a jogar) da posição
 * antes e depois dele. `beforeCp` é do POV de quem jogou; `afterCp` é do POV do
 * adversário (lado a jogar na posição resultante) — por isso o flip no winPctAfter.
 */
export function classifyVariationMove(
  beforeCp: number,
  afterCp: number,
): VariationMoveJudgement {
  const winPctBefore = cpToWinPct(beforeCp)
  const winPctAfter = 100 - cpToWinPct(afterCp)
  const winPctLoss = Math.max(0, winPctBefore - winPctAfter)
  return {
    winPctBefore,
    winPctAfter,
    winPctLoss,
    classification: classifyMove(winPctLoss, false),
  }
}

/**
 * Normaliza as linhas candidatas (multipv) de um RawPosition ao POV das brancas,
 * prontas para exibição. `stm` é o lado a jogar na posição avaliada.
 */
function normalizeLinesToWhite(
  rawLines: RawLine[] | undefined,
  stm: 'w' | 'b',
): PvLine[] {
  if (!rawLines) return []
  return rawLines.map((l) => ({
    multipv: l.multipv,
    san: l.san ?? null,
    cp: stm === 'w' ? l.cp : -l.cp,
    winPct: stm === 'w' ? cpToWinPct(l.cp) : 100 - cpToWinPct(l.cp),
    pv: l.pv,
  }))
}

/**
 * Aplica um resultado ao vivo do refino a um lance de variação pendente.
 * Preenche afterCp, depth, bestUci, lines (POV brancas) e o julgamento
 * (winPct/classification) derivado de `beforeCp` + `raw.cp`. Puro e idempotente:
 * refinos progressivos (depth crescente) apenas atualizam os mesmos campos.
 */
export function applyLiveToVariationMove(
  move: VariationMove,
  raw: RawPosition,
  beforeCp: number,
): VariationMove {
  const judgement = classifyVariationMove(beforeCp, raw.cp)
  // Após o lance, é a vez do adversário — esse é o lado a jogar da posição avaliada.
  const stm = move.color === 'w' ? 'b' : 'w'
  return {
    ...move,
    afterCp: raw.cp,
    depth: raw.depth,
    bestUci: raw.pv[0] ?? null,
    lines: normalizeLinesToWhite(raw.lines, stm),
    ...judgement,
  }
}

export type UserMoveDecision =
  | { kind: 'advance' }
  | { kind: 'variation'; parentPly: number }

/**
 * Decide o efeito de um lance arrastado pelo usuário no tabuleiro. Se ele
 * coincide com o próximo lance da linha principal (`nextMainlineUci`), apenas
 * avança a linha; caso contrário, abre/acrescenta numa variação ramificada do
 * `currentPly`. `nextMainlineUci === null` indica fim da linha principal.
 */
export function decideUserMove(
  uci: string,
  currentPly: number,
  nextMainlineUci: string | null,
): UserMoveDecision {
  if (nextMainlineUci !== null && nextMainlineUci === uci) {
    return { kind: 'advance' }
  }
  return { kind: 'variation', parentPly: currentPly }
}

/**
 * Resolve o cp "antes" de um lance de variação: para o primeiro lance, vem da
 * posição da linha principal de onde ramifica; para os demais, do afterCp do
 * lance anterior (que pode ainda ser indefinido → lance pendente).
 */
export function defaultBeforeCpResolver(
  variation: Variation,
  move: VariationMove,
  mainlineCpAt: (parentPly: number) => number | undefined,
): number | undefined {
  if (move.ply === 1) return mainlineCpAt(variation.parentPly)
  return variation.moves[move.ply - 2]?.afterCp
}

/**
 * Atualiza a análise de um lance-alvo no mapa de variações a partir de um
 * live-eval, sem mutar o estado original. Se o beforeCp (via `resolveBeforeCp`)
 * for indefinido ou o alvo não existir, devolve o mapa original (mesma ref) —
 * o lance segue pendente até que haja contexto para classificá-lo.
 */
export function applyLiveToVariation(
  variations: VariationMap,
  target: { variationId: string; moveId: string },
  raw: RawPosition,
  resolveBeforeCp: (
    variation: Variation,
    move: VariationMove,
  ) => number | undefined,
): VariationMap {
  for (const key of Object.keys(variations)) {
    const list = variations[Number(key)]
    const vIdx = list.findIndex((v) => v.id === target.variationId)
    if (vIdx === -1) continue
    const variation = list[vIdx]
    const mIdx = variation.moves.findIndex((m) => m.id === target.moveId)
    if (mIdx === -1) continue
    const move = variation.moves[mIdx]
    const beforeCp = resolveBeforeCp(variation, move)
    if (beforeCp === undefined) return variations
    const updatedMove = applyLiveToVariationMove(move, raw, beforeCp)
    const newMoves = [
      ...variation.moves.slice(0, mIdx),
      updatedMove,
      ...variation.moves.slice(mIdx + 1),
    ]
    const newVariation = { ...variation, moves: newMoves }
    const newList = [
      ...list.slice(0, vIdx),
      newVariation,
      ...list.slice(vIdx + 1),
    ]
    return { ...variations, [variation.parentPly]: newList }
  }
  return variations
}
