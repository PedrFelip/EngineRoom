import { Chess } from 'chess.js'
import { Deferred, Effect, Exit, Metric } from 'effect'
import { bestEffort, metrics } from '../effect/diagnostics'
import {
  type EngineError,
  EngineExitedError,
  EngineTimeoutError,
  InvalidPayloadError,
  MissingEvaluationError,
} from '../effect/errors'

import type { InfoScore } from '../uci'
import { isReadyOk, isUciOk, parseInfo, scoreToCp } from '../uci'
import type {
  AnalyzeControl,
  EngineExitReason,
  EnginePort,
  PlayedMove,
  RawLine,
  RawPosition,
} from './analysis-types'

export { MissingEvaluationError } from '../effect/errors'

interface ExtractedGame {
  positionFens: string[]
  moves: PlayedMove[]
}

export function extractGame(pgn: string): ExtractedGame {
  const chess = new Chess()
  chess.loadPgn(pgn)
  const verbose = chess.history({ verbose: true })
  const replay = new Chess()
  const positionFens: string[] = [replay.fen()]
  const moves: PlayedMove[] = []
  verbose.forEach((m, i) => {
    const fenBefore = replay.fen()
    replay.move({ from: m.from, to: m.to, promotion: m.promotion })
    positionFens.push(replay.fen())
    moves.push({
      ply: i + 1,
      color: m.color,
      san: m.san,
      uci: m.from + m.to + (m.promotion ?? ''),
      fenBefore,
    })
  })
  return { positionFens, moves }
}

/** Faz o handshake UCI e configura Threads, Hash e MultiPV. */
export function configureEngine(
  port: EnginePort,
  opts: {
    threads?: number
    hashMb?: number
    multipv: number
    timeoutMs?: number
  },
): Effect.Effect<void, EngineError> {
  return Effect.gen(function* () {
    const timeoutMs = opts.timeoutMs ?? 10_000
    yield* ask(port, 'uci', isUciOk, timeoutMs)
    yield* ask(port, 'isready', isReadyOk, timeoutMs)
    if (opts.threads && opts.threads > 1) {
      yield* port.send(`setoption name Threads value ${opts.threads}`)
    }
    if (opts.hashMb && opts.hashMb > 0) {
      yield* port.send(`setoption name Hash value ${opts.hashMb}`)
    }
    yield* port.send(
      `setoption name Multipv value ${Math.max(1, opts.multipv)}`,
    )
    yield* ask(port, 'isready', isReadyOk, timeoutMs)
  })
}

/** One scoped subscription per request. Parse callbacks synchronously so a
 * burst of UCI info does not allocate a stream chunk/fiber for every line. */
export function ask(
  port: EnginePort,
  cmd: string,
  done: (line: string) => boolean,
  timeoutMs = 10_000,
  dispatch?: Effect.Effect<void, EngineError>,
): Effect.Effect<void, EngineError> {
  return Effect.scoped(
    Effect.gen(function* () {
      const response = yield* Deferred.make<void, EngineError>()
      let finished = false
      const finish = (result: Effect.Effect<void, EngineError>) => {
        if (finished) return
        finished = true
        Deferred.unsafeDone(response, result)
      }
      yield* Effect.acquireRelease(
        Effect.sync(() =>
          port.onLine((line) => {
            if (finished) return
            try {
              if (done(line)) finish(Effect.void)
            } catch (defect) {
              finish(Effect.die(defect))
            }
          }),
        ),
        (off) =>
          Effect.sync(() => {
            finished = true
            off()
          }),
      )
      if (port.onExit) {
        yield* Effect.acquireRelease(
          Effect.sync(
            () =>
              port.onExit?.((reason) => {
                finish(
                  Effect.fail(
                    new EngineExitedError({
                      message: formatEngineExit(cmd, reason),
                      code: reason.code,
                      signal: reason.signal,
                    }),
                  ),
                )
              }) ?? (() => {}),
          ),
          (off) => Effect.sync(off),
        )
      }
      if (!finished) yield* dispatch ?? port.send(cmd)
      yield* Deferred.await(response)
    }),
  ).pipe(
    Effect.timeoutFail({
      duration: timeoutMs,
      onTimeout: () =>
        new EngineTimeoutError({
          message: `A engine não respondeu a '${cmd}' em ${timeoutMs}ms.`,
          command: cmd,
          timeoutMs,
        }),
    }),
    Effect.tapErrorTag('EngineTimeoutError', () =>
      Metric.increment(metrics.timeouts),
    ),
  )
}

/** Formata a mensagem de erro quando a engine encerra durante um comando. */
function formatEngineExit(cmd: string, reason: EngineExitReason): string {
  let detail = ''
  if (reason.error) detail = `: ${reason.error}`
  else if (reason.signal !== null) detail = ` (sinal ${reason.signal})`
  else if (reason.code !== null) detail = ` (código ${reason.code})`
  return `A engine encerrou durante '${cmd}'${detail}.`
}

export function uciToSan(fen: string, uci: string): string | null {
  try {
    const c = new Chess(fen)
    const m = c.move({
      from: uci.slice(0, 2),
      to: uci.slice(2, 4),
      promotion: uci[4],
    })
    return m ? m.san : null
  } catch {
    return null
  }
}

export function evalPosition(
  port: EnginePort,
  fen: string,
  control: AnalyzeControl,
  goTimeoutMs: number,
): Effect.Effect<RawPosition, EngineError> {
  return Effect.gen(function* () {
    const byPv = new Map<
      number,
      { depth: number; score?: InfoScore; pv: string[] }
    >()
    const goCmd =
      control.mode === 'depth'
        ? `go depth ${control.depth}`
        : `go movetime ${control.movetimeMs}`
    const positionCmd = `position fen ${fen}`
    const dispatch = port.sendBatch
      ? port.sendBatch([positionCmd, goCmd])
      : undefined
    if (!dispatch) yield* port.send(positionCmd)
    yield* ask(
      port,
      goCmd,
      (line) => {
        const info = parseInfo(line)
        if (info?.score) {
          const idx = info.multipv ?? 1
          const prev = byPv.get(idx)
          if (!prev || (info.depth ?? 0) >= prev.depth) {
            byPv.set(idx, {
              depth: info.depth ?? 0,
              score: info.score,
              pv: info.pv ?? [],
            })
          }
        }
        return line.trim().startsWith('bestmove')
      },
      goTimeoutMs,
      dispatch,
    ).pipe(
      Effect.onExit((exit) =>
        Exit.isFailure(exit)
          ? bestEffort(port.send('stop'), 'engine.stop_search')
          : Effect.void,
      ),
    )
    const lines: RawLine[] = [...byPv.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([multipv, l]) => ({
        multipv,
        cp: scoreToCp(l.score) ?? 0,
        pv: l.pv,
        depth: l.depth,
      }))
    const principal = lines.find((l) => l.multipv === 1) ?? lines[0]
    if (!principal) {
      return yield* new MissingEvaluationError()
    }
    return {
      fen,
      cp: principal.cp,
      depth: byPv.get(1)?.depth ?? 0,
      pv: principal.pv,
      lines,
    }
  }).pipe(
    Effect.tap(() => Metric.increment(metrics.positions)),
    Metric.trackDuration(metrics.duration),
  )
}

export function addSanToLines(pos: RawPosition): void {
  for (const line of pos.lines ?? []) {
    line.san = line.pv[0] ? uciToSan(pos.fen, line.pv[0]) : null
  }
}

export function terminalPosition(fen: string, cp: number): RawPosition {
  return {
    fen,
    cp,
    depth: 0,
    pv: [],
    lines: [{ multipv: 1, cp, pv: [] }],
  }
}

export function terminalCp(fen: string): number | null {
  try {
    const c = new Chess(fen)
    if (c.isCheckmate()) return -100000
    if (c.isGameOver()) return 0
    return null
  } catch {
    return null
  }
}

/** Calcula uma vez os terminais usados pelos loops e pelos orçamentos. */
export function terminalCps(fens: string[]): (number | null)[] {
  return fens.map(terminalCp)
}

export function extractGameEffect(pgn: string) {
  return Effect.try({
    try: () => extractGame(pgn),
    catch: (cause) =>
      new InvalidPayloadError({
        source: 'PGN',
        cause,
        message: 'PGN inválido para análise.',
      }),
  })
}
