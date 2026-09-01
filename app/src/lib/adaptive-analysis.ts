import { Chess } from 'chess.js'
import type { AnalysisKind } from '../types'
import type { PlayedMove, RawPosition } from './analyze'
import { cpToWinPct } from './scoring'

export type AdaptiveProfileId = 'fast' | 'deep'

export interface AdaptiveProfile {
  id: AdaptiveProfileId
  label: string
  /** Busca ampla: curta, mas já com alternativas para estimar a decisão. */
  triageMs: number
  triageMultipv: number
  mediumMs: number
  highMs: number
  refinementMultipv: number
  /** Limite das posições refinadas; gatilhos duros não são descartados. */
  maxRefineFraction: number
  minRefinePositions: number
}

export const ADAPTIVE_PROFILES: Record<AdaptiveProfileId, AdaptiveProfile> = {
  fast: {
    id: 'fast',
    label: 'Automático rápido',
    triageMs: 120,
    triageMultipv: 2,
    mediumMs: 600,
    highMs: 1_500,
    refinementMultipv: 2,
    maxRefineFraction: 0.2,
    minRefinePositions: 6,
  },
  deep: {
    id: 'deep',
    label: 'Automático profundo',
    triageMs: 300,
    triageMultipv: 3,
    mediumMs: 1_500,
    highMs: 4_000,
    refinementMultipv: 3,
    maxRefineFraction: 0.35,
    minRefinePositions: 10,
  },
}

export function adaptiveProfileForKind(
  kind: AnalysisKind | undefined,
): AdaptiveProfile | null {
  if (kind === 'auto-fast') return ADAPTIVE_PROFILES.fast
  if (kind === 'auto-deep') return ADAPTIVE_PROFILES.deep
  return null
}

export interface CriticalMove {
  ply: number
  score: number
  hard: boolean
  reasons: string[]
}

export interface RefinementTarget {
  positionIndex: number
  score: number
  budget: 'medium' | 'high'
}

function clampScore(value: number, max: number): number {
  return Math.min(max, Math.max(0, value))
}

function materialComplexity(fen: string): {
  legalMoves: number
  nonPawnPieces: number
  inCheck: boolean
} {
  try {
    const chess = new Chess(fen)
    const nonPawnPieces = chess
      .board()
      .flat()
      .filter(
        (piece) => piece && piece.type !== 'p' && piece.type !== 'k',
      ).length
    return {
      legalMoves: chess.moves().length,
      nonPawnPieces,
      inCheck: chess.inCheck(),
    }
  } catch {
    return { legalMoves: 0, nonPawnPieces: 0, inCheck: false }
  }
}

function multipvGap(before: RawPosition): number | null {
  const first = before.lines?.find((line) => line.multipv === 1)
  const second = before.lines?.find((line) => line.multipv === 2)
  if (!first || !second) return null
  return Math.abs(first.cp - second.cp)
}

/**
 * Classifica lances a partir da triagem. A pontuação escolhe orçamento, não a
 * classificação final: ela combina impacto, tática, complexidade e incerteza.
 */
export function rankCriticalMoves(
  moves: PlayedMove[],
  raw: RawPosition[],
  bookMaxPly = 0,
): CriticalMove[] {
  return moves.map((move) => {
    const before = raw[move.ply - 1]
    const after = raw[move.ply]
    if (!before || !after || move.ply <= bookMaxPly) {
      return {
        ply: move.ply,
        score: 0,
        hard: false,
        reasons: [],
      }
    }

    const winBefore = cpToWinPct(before.cp)
    const winAfter = 100 - cpToWinPct(after.cp)
    const loss = Math.max(0, winBefore - winAfter)
    const swing = Math.abs(winBefore - winAfter)
    const gap = multipvGap(before)
    const complexity = materialComplexity(move.fenBefore)
    const isCapture = move.san.includes('x')
    const givesCheck = move.san.includes('+') || move.san.includes('#')
    const promotes = move.san.includes('=')
    const mateSignal =
      Math.abs(before.cp) >= 90_000 || Math.abs(after.cp) >= 90_000
    let score = 0
    const reasons: string[] = []

    score += clampScore((loss / 20) * 24, 24)
    score += clampScore((swing / 20) * 11, 11)
    if (loss >= 5) reasons.push('perda de avaliação')
    if (swing >= 10) reasons.push('virada de avaliação')

    if (isCapture) score += 4
    if (givesCheck) score += 7
    if (promotes) score += 12
    if (complexity.inCheck) score += 7
    if (isCapture || givesCheck || promotes) {
      reasons.push('sequência tática')
    }

    score += clampScore(((complexity.legalMoves - 18) / 22) * 10, 10)
    score += clampScore((complexity.nonPawnPieces / 10) * 8, 8)
    if (complexity.legalMoves >= 30 || complexity.nonPawnPieces >= 8) {
      reasons.push('posição complexa')
    }

    if (gap !== null) {
      score += clampScore((gap / 180) * 12, 12)
      if (gap >= 80) reasons.push('melhor lance se destaca')
    }

    const nearClassificationBoundary = [2, 5, 10, 20].some(
      (boundary) => Math.abs(loss - boundary) <= 1.25,
    )
    if (nearClassificationBoundary) {
      score += 8
      reasons.push('classificação incerta')
    }

    if (mateSignal) {
      score += 20
      reasons.push('sequência de mate')
    }

    const hard =
      loss >= 10 ||
      swing >= 15 ||
      mateSignal ||
      promotes
    return {
      ply: move.ply,
      score: Math.min(100, Math.round(score)),
      hard,
      reasons,
    }
  })
}

/**
 * Converte lances prioritários nas posições realmente necessárias. Para
 * classificar um lance, refinamos os dois lados da comparação: antes e depois.
 */
export function selectRefinementTargets(
  criticalMoves: CriticalMove[],
  positionCount: number,
  profile: AdaptiveProfile,
): RefinementTarget[] {
  const ranked = criticalMoves
    .filter((move) => move.score >= 32 || move.hard)
    .sort((a, b) => Number(b.hard) - Number(a.hard) || b.score - a.score)
  const softLimit = Math.max(
    profile.minRefinePositions,
    Math.ceil(positionCount * profile.maxRefineFraction),
  )
  const targets = new Map<number, RefinementTarget>()

  for (const move of ranked) {
    const indexes = [move.ply - 1, move.ply]
    const addsNewPosition = indexes.some((index) => !targets.has(index))
    if (!move.hard && addsNewPosition && targets.size >= softLimit) continue

    for (const positionIndex of indexes) {
      if (positionIndex < 0 || positionIndex >= positionCount) continue
      const budget = move.hard || move.score >= 65 ? 'high' : 'medium'
      const previous = targets.get(positionIndex)
      if (
        !previous ||
        previous.score < move.score ||
        (previous.budget === 'medium' && budget === 'high')
      ) {
        targets.set(positionIndex, {
          positionIndex,
          score: move.score,
          budget,
        })
      }
    }
  }

  return [...targets.values()].sort(
    (a, b) =>
      Number(b.budget === 'high') - Number(a.budget === 'high') ||
      b.score - a.score,
  )
}
