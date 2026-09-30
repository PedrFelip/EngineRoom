// Test reference only. Frozen before switching production to Rust.
import { writeFileSync, mkdirSync } from 'node:fs'
import { Chess } from 'chess.js'
import { extractGame, addSanToLines } from '../src/lib/__tests__/reference/analysis/engine-analysis'
import { buildReview } from '../src/lib/__tests__/reference/analysis/review-builder'
import { lookupEco } from '../src/lib/eco'
import { ADAPTIVE_PROFILES, rankCriticalMoves, selectRefinementTargets } from '../src/lib/adaptive-analysis'
import dataset from '../src/data/eco.json'
const pgns = [
  '1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7',
  '1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7',
  '1. f3 e5 2. g4 Qh4#',
  '1. e4 a6 2. e5 d5 3. exd6 exd6 4. Nf3',
]
const cases = pgns.map((pgn) => {
  const { positionFens, moves } = extractGame(pgn)
  const scores = [15, -50, 150, -400, 200, -900, 0, 1200, -300, 100000, -99997]
  const raw = positionFens.map((fen, i) => {
    const chess = new Chess(fen)
    const candidates = chess.moves({ verbose: true }).slice(0, 3)
    const lines = candidates.map((m, k) => ({ multipv: k + 1, cp: scores[i % scores.length] - k * 95, depth: 14, pv: [m.from + m.to + (m.promotion ?? '')] }))
    if (!lines.length) lines.push({ multipv: 1, cp: -100000, depth: 0, pv: [] })
    const pos = { fen, cp: lines[0].cp, depth: lines[0].depth, pv: lines[0].pv, lines }
    addSanToLines(pos)
    return pos
  })
  const eco = lookupEco(moves.map((m) => m.san), dataset)
  const book = eco ? { maxPly: eco.moves.length, eco } : undefined
  const critical = rankCriticalMoves(moves, raw, book?.maxPly ?? 0)
  return { pgn, moves, raw, expected: buildReview({ startFen:positionFens[0],moves }, raw, book), critical, targets:selectRefinementTargets(critical,raw.length,ADAPTIVE_PROFILES.fast) }
})
mkdirSync('src-tauri/src/review/fixtures', { recursive:true })
writeFileSync('src-tauri/src/review/fixtures/parity.json', JSON.stringify(cases,null,2) + '\n')
