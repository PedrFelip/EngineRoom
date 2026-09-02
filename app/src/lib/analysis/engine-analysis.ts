import { Chess } from 'chess.js'
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
export async function configureEngine(
  port: EnginePort,
  opts: {
    threads?: number
    hashMb?: number
    multipv: number
    timeoutMs?: number
  },
): Promise<void> {
  const timeoutMs = opts.timeoutMs ?? 10_000
  await ask(port, 'uci', isUciOk, timeoutMs)
  await ask(port, 'isready', isReadyOk, timeoutMs)
  if (opts.threads && opts.threads > 1) {
    await port.send(`setoption name Threads value ${opts.threads}`)
  }
  if (opts.hashMb && opts.hashMb > 0) {
    await port.send(`setoption name Hash value ${opts.hashMb}`)
  }
  await port.send(`setoption name Multipv value ${Math.max(1, opts.multipv)}`)
}

/** Aguarda uma resposta UCI, falhando por timeout ou término da engine. */
export function ask(
  port: EnginePort,
  cmd: string,
  done: (line: string) => boolean,
  timeoutMs = 10_000,
): Promise<void> {
  return new Promise((resolve, reject) => {
    let off: () => void = () => {}
    let offExit: () => void = () => {}
    let timer: ReturnType<typeof setTimeout>
    const cleanup = () => {
      off()
      offExit()
      clearTimeout(timer)
    }
    off = port.onLine((line) => {
      if (done(line)) {
        cleanup()
        resolve()
      }
    })
    offExit =
      port.onExit?.((reason) => {
        cleanup()
        reject(new Error(formatEngineExit(cmd, reason)))
      }) ?? (() => {})
    timer = setTimeout(() => {
      cleanup()
      reject(new Error(`A engine não respondeu a '${cmd}' em ${timeoutMs}ms.`))
    }, timeoutMs)
    void port.send(cmd)
  })
}

/** Formata a mensagem de erro quando a engine encerra durante um comando. */
function formatEngineExit(cmd: string, reason: EngineExitReason): string {
  const detail = reason.error
    ? `: ${reason.error}`
    : reason.signal !== null
      ? ` (sinal ${reason.signal})`
      : reason.code !== null
        ? ` (código ${reason.code})`
        : ''
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

export async function evalPosition(
  port: EnginePort,
  fen: string,
  control: AnalyzeControl,
  goTimeoutMs: number,
): Promise<RawPosition> {
  const byPv = new Map<
    number,
    { depth: number; score?: InfoScore; pv: string[] }
  >()
  await port.send(`position fen ${fen}`)
  const goCmd =
    control.mode === 'depth'
      ? `go depth ${control.depth}`
      : `go movetime ${control.movetimeMs}`
  try {
    await ask(
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
    )
  } catch (err) {
    // Aborta a busca órfã para que a engine volte a ficar reutilizável.
    await port.send('stop')
    throw err
  }
  const lines: RawLine[] = [...byPv.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([multipv, l]) => ({
      multipv,
      cp: scoreToCp(l.score) ?? 0,
      pv: l.pv,
      depth: l.depth,
    }))
  const principal = lines.find((l) => l.multipv === 1) ?? lines[0]
  return {
    fen,
    cp: principal?.cp ?? 0,
    depth: byPv.get(1)?.depth ?? 0,
    pv: principal?.pv ?? [],
    lines,
  }
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
