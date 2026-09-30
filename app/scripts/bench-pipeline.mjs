// Shared workload with Rust's benchmark_complete_pipeline test. No production imports.
import { readFileSync, writeFileSync } from 'node:fs'
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { Effect, Fiber, Exit } from 'effect'
import { analyzeGame, analyzeGameAdaptive } from '../src/lib/__tests__/reference/analyze'
import { evalPosition } from '../src/lib/__tests__/reference/analysis/engine-analysis'

const fixture = JSON.parse(readFileSync('src-tauri/src/review/fixtures/benchmark.json', 'utf8'))
const samples = Number(process.env.BENCH_SAMPLES ?? fixture.samples)
const warmups = fixture.warmups

function fakePort(game, multipv, stalled = false) {
  const scores = new Map(game.raw.map((raw) => [raw.fen, raw]))
  const stats = { searches: 0, infoLines: 0, progress: 0, cacheLookups: 0, cacheWrites: 0, cacheEntries: 0 }
  let handler = () => {}
  let fen = game.raw[0].fen
  return {
    stats,
    send: (command) => Effect.sync(() => {
      if (command === 'uci') handler('uciok')
      if (command === 'isready') handler('readyok')
      if (command.startsWith('position fen ')) fen = command.slice(13)
      if (command.toLowerCase().startsWith('setoption name multipv value ')) multipv = Number(command.split(' ').at(-1))
      if (command.startsWith('go ')) {
        stats.searches++
        if (!stalled) {
          for (const line of scores.get(fen).lines.slice(0, multipv)) {
            for (let depth = 1; depth <= 32; depth++) {
              stats.infoLines++
              handler(`info depth ${depth} multipv ${line.multipv} score cp ${line.cp} pv ${line.pv.join(' ')}`)
            }
          }
          handler(`bestmove ${scores.get(fen).pv[0] ?? '(none)'}`)
        }
      }
      if (command === 'stop') handler('bestmove (none)')
    }),
    onLine(callback) { handler = callback; return () => { handler = () => {} } },
  }
}

function cacheFor(game, scenario, stats) {
  const hits = new Map(game.raw.filter((_, i) => scenario.cache === 'warm' || (scenario.cache === 'mixed' && i % 2 === 0)).map((pos) => [pos.fen, pos]))
  return {
    getBulk: (fens, _mode, _value, multipv) => Effect.sync(() => {
      stats.cacheLookups++
      return fens.map((fen) => {
        const hit = hits.get(fen)
        return hit ? { ...hit, lines: hit.lines.slice(0, multipv).map((l) => ({ ...l })) } : null
      })
    }),
    // Cold cache remains cold between runs, matching the Rust injected repository.
    putMany: (entries) => Effect.sync(() => {
      stats.cacheWrites++
      stats.cacheEntries += entries.length
    }),
  }
}

function realPort() {
  const child = spawn(process.env.BENCH_STOCKFISH ?? 'src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu', [], { stdio: ['pipe', 'pipe', 'pipe'] })
  const stats = { searches: 0, infoLines: 0, progress: 0, cacheLookups: 0, cacheWrites: 0, cacheEntries: 0 }
  let handler = () => {}
  let exitHandler = () => {}
  const lines = createInterface({ input: child.stdout })
  lines.on('line', (line) => {
    if (line.startsWith('info depth ')) stats.infoLines++
    handler(line)
  })
  const closed = new Promise((resolve) => child.once('exit', (code, signal) => {
    exitHandler({ code, signal: signal ? 1 : null })
    resolve()
  }))
  child.once('error', (error) => exitHandler({ code: null, signal: null, error: String(error) }))
  // Consume stderr so the child cannot block on a full pipe.
  child.stderr.resume()
  return {
    stats,
    send: (command) => Effect.sync(() => {
      if (command.startsWith('go ')) stats.searches++
      child.stdin.write(command + '\n')
    }),
    onLine(callback) { handler = callback; return () => { handler = () => {} } },
    onExit(callback) { exitHandler = callback; return () => { exitHandler = () => {} } },
    async close() {
      if (child.exitCode === null && child.signalCode === null) child.kill()
      await closed
      lines.close()
    },
  }
}

async function execute(scenario, game) {
  const real = scenario.engine === 'stockfish'
  const port = real ? realPort() : fakePort(game, scenario.multipv, scenario.task === 'cancel')
  const control = scenario.mode === 'depth' ? { mode: 'depth', depth: real ? 8 : 20 } : { mode: 'time', movetimeMs: real ? 20 : 120 }
  const opts = { threads: 1, hashMb: 16, cache: cacheFor(game, scenario, port.stats), onDetailedProgress: () => { port.stats.progress++ } }
  try {
  let result
  if (scenario.task === 'search') {
    // One Effect runtime for the whole batch, as in the original search benchmark.
    result = await Effect.runPromise(Effect.gen(function* () {
      let last
      for (let i = 0; i < scenario.batch; i++) last = yield* evalPosition(port, game.raw[0].fen, control, 10000)
      return last
    }))
  } else if (scenario.task === 'cancel') {
    for (let i = 0; i < scenario.batch; i++) {
      const pending = Effect.runFork(evalPosition(port, game.raw[0].fen, control, 10000))
      await Effect.runPromise(Effect.yieldNow())
      const exit = await Effect.runPromise(Fiber.interrupt(pending))
      if (!Exit.isInterrupted(exit)) throw new Error('Search was not interrupted')
    }
    result = { cancelled: true }
  } else {
    for (let i = 0; i < scenario.batch; i++) {
      result = await Effect.runPromise(scenario.kind === 'manual'
        ? analyzeGame(game.pgn, control, port, scenario.multipv, opts)
        : analyzeGameAdaptive(game.pgn, scenario.kind, port, opts))
    }
  }
  return { result, stats: port.stats }
  } finally {
    if (real) await port.close()
  }
}

const runtime = globalThis.Bun ? `Bun ${Bun.version}` : `Node ${process.version}`
const collect = () => globalThis.Bun ? Bun.gc(true) : globalThis.gc?.()
const report = { runtime, warmups, samples, scenarios: [] }
for (const scenario of fixture.scenarios) {
  if (process.env.BENCH_ENGINE === 'fake' && scenario.engine === 'stockfish') continue
  if (process.env.BENCH_ENGINE === 'stockfish' && scenario.engine !== 'stockfish') continue
  const game = fixture.games[scenario.game]
  process.stderr.write(`TS ${scenario.id}\n`)
  try {
    const count = Number(process.env.BENCH_SAMPLES ?? scenario.samples ?? samples)
    for (let i = 0; i < (scenario.warmups ?? warmups); i++) await execute(scenario, game)
    collect()
    const heapBefore = process.memoryUsage().heapUsed
    const elapsedMs = []
    let verification
    for (let i = 0; i < count; i++) {
      const start = performance.now()
      const value = await execute(scenario, game)
      elapsedMs.push((performance.now() - start) / scenario.batch)
      verification = value
    }
    collect()
    report.scenarios.push({ ...scenario, runtime, unit: scenario.task === 'game' ? 'game' : 'search', elapsedMs, retainedHeapDeltaBytes: process.memoryUsage().heapUsed - heapBefore, ...verification })
  } catch (error) {
    // Record limitations of the frozen baseline without silently modifying it.
    report.scenarios.push({ ...scenario, error: String(error) })
  }
}
const output = JSON.stringify(report, null, 2) + '\n'
if (process.env.BENCH_REPORT_PATH) writeFileSync(process.env.BENCH_REPORT_PATH, output)
else console.log(output)
