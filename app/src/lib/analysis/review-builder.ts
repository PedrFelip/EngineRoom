import type {
  AccuracyByColor,
  MoveAnalysis,
  Phase,
  PositionAnalysis,
  PvLine,
  ReviewResult,
} from '../../types'
import { computePhases } from '../phase'
import {
  ACCURACY_MODEL_VERSION,
  centipawnLoss,
  classifyMove,
  cpToWinPct,
  gameAccuracy,
  sideToMoveAtPly,
  whiteCp,
  whiteWinPct,
} from '../scoring'
import type { BookInfo, PlayedGame, RawPosition } from './analysis-types'

/**
 * Constrói a revisão a partir da partida jogada e das avaliações brutas por ply.
 * `raw[i]` é a avaliação da posição após o i-ésimo ply (raw[0] = posição inicial).
 * O win% das posições é normalizado para o ponto de vista das brancas.
 */
export function buildReview(
  game: PlayedGame,
  raw: RawPosition[],
  book?: BookInfo,
): ReviewResult {
  const phases = computePhases(raw.map((r) => ({ fen: r.fen })))

  const positions: PositionAnalysis[] = raw.map((r, i) => {
    const stm = sideToMoveAtPly(game.moves, i)
    const winPct = whiteWinPct(r.cp, stm)
    const rawLines = r.lines ?? [{ multipv: 1, cp: r.cp, pv: r.pv }]
    const lines: PvLine[] = rawLines.map((l) => ({
      multipv: l.multipv,
      san: l.san ?? null,
      cp: whiteCp(l.cp, stm),
      winPct: whiteWinPct(l.cp, stm),
      pv: l.pv,
    }))
    return {
      ply: i,
      fen: r.fen,
      phase: phases[i],
      depth: r.depth,
      cp: r.cp,
      winPct,
      pv: r.pv,
      lines,
    }
  })

  const moves: MoveAnalysis[] = game.moves.map((m) => {
    const before = raw[m.ply - 1]
    const after = raw[m.ply]
    const winPctBefore = cpToWinPct(before.cp)
    const winPctAfter = 100 - cpToWinPct(after.cp)
    const winPctLoss = Math.max(0, winPctBefore - winPctAfter)
    const cpLoss = centipawnLoss(before.cp, after.cp)
    const isBook = !!book && m.ply <= book.maxPly
    const classification = classifyMove(winPctLoss, isBook)
    return {
      ply: m.ply,
      color: m.color,
      san: m.san,
      uci: m.uci,
      fenBefore: m.fenBefore,
      classification,
      winPctBefore,
      winPctAfter,
      winPctLoss,
      cpLoss,
      bestUci: before.pv[0] ?? null,
      isBook,
      eco:
        isBook && book?.eco
          ? { code: book.eco.code, name: book.eco.name }
          : null,
    }
  })

  const positionWinPcts = positions.map((position) => position.winPct)
  const accuracy: AccuracyByColor = gameAccuracy(moves, positionWinPcts)

  const accuracyByPhase = accuracyByPhaseOf(moves, phases, positionWinPcts)

  return {
    positions,
    moves,
    accuracyModel: ACCURACY_MODEL_VERSION,
    accuracy,
    accuracyByPhase,
  }
}

/**
 * Acurácia agregada (0–100) por fase do jogo. Um lance pertence à fase da
 * posição de onde partiu (`phases[ply - 1]`). Cada fase reaplica o agregador
 * completo sobre seu trecho, seguindo `phaseAccuracies` do Lichess. Lances de
 * livro continuam incluídos.
 */
export function accuracyByPhaseOf(
  moves: MoveAnalysis[],
  phases: Phase[],
  positionWinPcts: number[],
): Record<Phase, AccuracyByColor> {
  const forPhase = (phase: Phase): AccuracyByColor => {
    const phaseMoves = moves.filter((move) => phases[move.ply - 1] === phase)
    if (phaseMoves.length === 0) return gameAccuracy([], [50])
    const firstPosition = phaseMoves[0].ply - 1
    const phaseWinPcts = [
      positionWinPcts[firstPosition],
      ...phaseMoves.map((move) => positionWinPcts[move.ply]),
    ]
    return gameAccuracy(phaseMoves, phaseWinPcts)
  }
  return {
    opening: forPhase('opening'),
    middlegame: forPhase('middlegame'),
    endgame: forPhase('endgame'),
  }
}
