// bun run scripts/bench-effect.mjs [/absolute/path/to/baseline/app/src]
// Run in a quiet machine. Fake UCI only: these numbers exclude Stockfish CPU.
import { Effect, Fiber } from 'effect'
import { pathToFileURL } from 'node:url'
import { analyzeGame } from '../src/lib/analyze.ts'
import { evalPosition } from '../src/lib/analysis/engine-analysis.ts'

const fen = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const pgn = '1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7'
const samples = 11
const searches = 300

function fakePort(nativeEffect, stalled = false) {
  let handler = () => {}
  const send = (command) => {
    if (command === 'uci') handler('uciok')
    if (command === 'isready') handler('readyok')
    if ((!stalled && command.startsWith('go ')) || command === 'stop') {
      for (let depth = 1; depth <= 32; depth++) {
        handler(`info depth ${depth} multipv 1 score cp 15 pv e2e4 e7e5`)
      }
      handler('bestmove e2e4')
    }
  }
  return {
    send: nativeEffect ? (command) => Effect.sync(() => send(command)) : send,
    onLine(cb) { handler = cb; return () => { handler = () => {} } },
  }
}
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)]
async function measure(run) {
  for (let i = 0; i < 3; i++) await run()
  const elapsed = []
  globalThis.Bun?.gc(true)
  const before = process.memoryUsage().heapUsed
  for (let i = 0; i < samples; i++) {
    const start = performance.now()
    await run()
    elapsed.push(performance.now() - start)
  }
  globalThis.Bun?.gc(true)
  return {
    medianMs: Number(median(elapsed).toFixed(3)),
    retainedHeapDeltaBytes: process.memoryUsage().heapUsed - before,
  }
}
const port = fakePort(true)
const effectSearches = Effect.gen(function* () {
  for (let i = 0; i < searches; i++) {
    yield* evalPosition(port, fen, { mode: 'depth', depth: 20 }, 1000)
  }
})
const report = {
  runtime: process.version, samples, searches, linesPerSearch: 33,
  effect: {
    searches: await measure(() => Effect.runPromise(effectSearches)),
    gameWithProgress: await measure(() => Effect.runPromise(analyzeGame(
      pgn, { mode: 'depth', depth: 20 }, port, 1,
      { onDetailedProgress: () => {} },
    ))),
    interruption: await measure(async () => {
      const pending = Effect.runFork(evalPosition(fakePort(true, true), fen, { mode: 'time', movetimeMs: 1000 }, 11000))
      await Effect.runPromise(Effect.yieldNow())
      await Effect.runPromise(Fiber.interrupt(pending))
    }),
  },
}
if (process.argv[2]) {
  const source = `${process.argv[2]}/lib`
  const oldAnalysis = await import(pathToFileURL(`${source}/analyze.ts`).href)
  const oldEngine = await import(pathToFileURL(`${source}/analysis/engine-analysis.ts`).href)
  const oldPort = fakePort(false)
  const baselineResult = await oldAnalysis.analyzeGame(pgn, { mode: 'depth', depth: 20 }, oldPort)
  const nextResult = await Effect.runPromise(analyzeGame(pgn, { mode: 'depth', depth: 20 }, port))
  if (JSON.stringify(baselineResult) !== JSON.stringify(nextResult)) {
    throw new Error('Benchmark fixture changed its review result')
  }
  report.baseline = {
    searches: await measure(async () => {
      for (let i = 0; i < searches; i++) {
        await oldEngine.evalPosition(oldPort, fen, { mode: 'depth', depth: 20 }, 1000)
      }
    }),
    gameWithProgress: await measure(() => oldAnalysis.analyzeGame(
      pgn, { mode: 'depth', depth: 20 }, oldPort, 1,
      { onDetailedProgress: () => {} },
    )),
  }
}
console.log(JSON.stringify(report, null, 2))
