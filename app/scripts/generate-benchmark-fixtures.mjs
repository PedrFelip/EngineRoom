import { readFileSync, writeFileSync } from 'node:fs'
import { Chess } from 'chess.js'
import { addSanToLines } from '../src/lib/__tests__/reference/analysis/engine-analysis'

let seed = 42
const chess = new Chess()
for (let ply = 0; ply < 80 && !chess.isGameOver(); ply++) {
  const moves = chess.moves()
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0
  chess.move(moves[seed % moves.length])
}
const pgns = {
  short: '1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7',
  medium: '1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O 9. h3 Nb8 10. d4 Nbd7 11. c4 c6 12. Nc3 Bb7 13. a3 Re8 14. Ba2 Bf8 15. Bg5 h6 16. Bh4 g5 17. Bg3 Nh5 18. Nxe5 dxe5 19. Qxh5 Qf6 20. cxb5 axb5',
  long: chess.pgn(),
  mate: '1. f3 e5 2. g4 Qh4#',
  promotion: '[SetUp "1"]\n[FEN "7k/P7/8/8/8/8/8/7K w - - 0 1"]\n1. a8=Q+',
  blackFen: '[SetUp "1"]\n[FEN "7k/8/8/8/8/8/8/R6K b - - 0 1"]\n1... Kg7 2. Ra7+',
}
const corpus = readFileSync('benchmarks/games.pgn', 'utf8').split(/(?=\[Event )/).filter((pgn) => pgn.trim())
for (const [i, pgn] of corpus.entries()) pgns[`real${i + 1}`] = pgn
pgns.long = pgns.real9
const games = Object.fromEntries(Object.entries(pgns).map(([id, pgn]) => {
  const board = new Chess()
  board.loadPgn(pgn)
  const history = board.history({ verbose: true })
  const positionFens = [history[0].before, ...history.map((m) => m.after)]
  const raw = positionFens.map((fen, i) => {
    const board = new Chess(fen)
    const cp = [15, -50, 150, -400, 200, -900, 0, 1200, -300][i % 9]
    const lines = board.moves({ verbose: true }).slice(0, 3).map((m, k) => ({
      multipv: k + 1, cp: cp - k * 95, depth: 32,
      pv: [m.from + m.to + (m.promotion ?? '')],
    }))
    const pos = { fen, cp, depth: 32, pv: lines[0]?.pv ?? [], lines }
    addSanToLines(pos)
    return pos
  })
  return [id, { pgn, raw, metadata: board.header(), plies: history.length }]
}))
const scenarios = []
for (const mode of ['depth', 'time']) {
  for (const multipv of [1, 3]) {
    scenarios.push({ id: `search-${mode}-pv${multipv}`, task: 'search', game: 'short', mode, multipv, cache: 'cold', kind: 'manual', batch: 300 })
  }
}
for (const game of Object.keys(games)) {
  scenarios.push({ id: `game-${game}-depth-pv1-cold`, task: 'game', game, mode: 'depth', multipv: 1, cache: 'cold', kind: 'manual', batch: 5 })
}
for (const cache of ['cold', 'warm', 'mixed']) {
  for (const mode of ['depth', 'time']) {
    scenarios.push({ id: `game-long-${mode}-pv3-${cache}`, task: 'game', game: 'long', mode, multipv: 3, cache, kind: 'manual', batch: 5 })
  }
  for (const kind of ['fast', 'deep']) {
    scenarios.push({ id: `adaptive-long-${kind}-${cache}`, task: 'game', game: 'long', mode: 'time', multipv: 3, cache, kind, batch: 5 })
  }
}
scenarios.push({ id: 'cancel-blocked-search', task: 'cancel', game: 'short', mode: 'time', multipv: 1, cache: 'cold', kind: 'manual', batch: 100 })
for (const game of ['real1', 'real10']) {
  for (const mode of ['depth', 'time']) {
    for (const multipv of [1, 3]) {
      scenarios.push({ id: `stockfish-${game}-${mode}-pv${multipv}`, task: 'game', game, mode, multipv, cache: 'cold', kind: 'manual', batch: 1, engine: 'stockfish', warmups: 1, samples: 5 })
    }
  }
}
writeFileSync('src-tauri/src/review/fixtures/benchmark.json', JSON.stringify({ warmups: 3, samples: 15, source: 'https://github.com/tonymorris/immortalgames/blob/master/immortal_games.pgn', games, scenarios }, null, 2) + '\n')
