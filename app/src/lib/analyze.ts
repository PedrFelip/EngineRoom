/**
 * Orquestração da revisão: converte uma partida jogada + avaliações brutas do
 * engine em um ReviewResult completo (win%, classificação de lances e precisão).
 *
 * `buildReview` é puro (sem engine, sem async) — toda a matemática de sinal,
 * perda de win% e classificação mora aqui. `analyzeGame` é a cola de I/O que
 * aciona o engine via um "port" injetável (testável com engine falso).
 */

import { Chess } from 'chess.js'
import type {
  AccuracyByColor,
  MoveAnalysis,
  PositionAnalysis,
  PvLine,
  ReviewResult,
} from '../types'
import { type EcoEntry, lookupOpening } from './eco'
import { classifyMove, cpToWinPct, gameAccuracy } from './scoring'
import type { InfoScore } from './uci'
import { isReadyOk, isUciOk, parseInfo, scoreToCp } from './uci'

export interface BookInfo {
  maxPly: number
  eco: EcoEntry | null
}

export interface RawLine {
  multipv: number
  cp: number
  pv: string[]
  san?: string | null
}

export interface RawPosition {
  fen: string
  cp: number
  depth: number
  pv: string[]
  lines?: RawLine[]
}

export interface PlayedMove {
  ply: number
  color: 'w' | 'b'
  san: string
  uci: string
  fenBefore: string
}

export interface PlayedGame {
  startFen: string
  moves: PlayedMove[]
}

const opposite = (c: 'w' | 'b'): 'w' | 'b' => (c === 'w' ? 'b' : 'w')

function sideToMoveAt(game: PlayedGame, ply: number): 'w' | 'b' {
  if (ply < game.moves.length) return game.moves[ply].color
  return opposite(game.moves[game.moves.length - 1].color)
}

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
  const positions: PositionAnalysis[] = raw.map((r, i) => {
    const stm = sideToMoveAt(game, i)
    const winPct = stm === 'w' ? cpToWinPct(r.cp) : 100 - cpToWinPct(r.cp)
    const rawLines = r.lines ?? [{ multipv: 1, cp: r.cp, pv: r.pv }]
    const lines: PvLine[] = rawLines.map((l) => ({
      multipv: l.multipv,
      san: l.san ?? null,
      cp: stm === 'w' ? l.cp : -l.cp,
      winPct: stm === 'w' ? cpToWinPct(l.cp) : 100 - cpToWinPct(l.cp),
      pv: l.pv,
    }))
    return {
      ply: i,
      fen: r.fen,
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
    const isBook = !!book && m.ply <= book.maxPly
    return {
      ply: m.ply,
      color: m.color,
      san: m.san,
      uci: m.uci,
      fenBefore: m.fenBefore,
      classification: classifyMove(winPctLoss, isBook),
      winPctBefore,
      winPctAfter,
      winPctLoss,
      bestUci: before.pv[0] ?? null,
      isBook,
      eco:
        isBook && book?.eco
          ? { code: book.eco.code, name: book.eco.name }
          : null,
    }
  })

  const accuracy: AccuracyByColor = {
    white: gameAccuracy(
      moves
        .filter((m) => m.color === 'w' && !m.isBook)
        .map((m) => m.winPctLoss),
    ),
    black: gameAccuracy(
      moves
        .filter((m) => m.color === 'b' && !m.isBook)
        .map((m) => m.winPctLoss),
    ),
  }

  return { positions, moves, accuracy }
}

/** Interface do engine injetável, para testar com engine falso. */
export interface EnginePort {
  send(cmd: string): void | Promise<void>
  onLine(handler: (line: string) => void): () => void
}

/**
 * Como a engine deve buscar cada posição:
 *  - `depth`: fixa em N ply (`go depth N`);
 *  - `time`: fixa em N ms (`go movetime N`), estilo chess.com "Maximum Time".
 */
export type AnalyzeControl =
  | { mode: 'depth'; depth: number }
  | { mode: 'time'; movetimeMs: number }

/**
 * Valor escalar usado como chave de cache: o `depth` no modo profundidade,
 * ou `movetimeMs` no modo tempo. A diferenciação entre modos é feita pelo
 * campo `mode` (também parte da chave), evitando colisão entre
 * `depth=20` e `movetimeMs=20`, por exemplo.
 */
export function controlKeyValue(control: AnalyzeControl): number {
  return control.mode === 'depth' ? control.depth : control.movetimeMs
}

/**
 * Orçamento padrão de timeout por posição para o comando `go`:
 *  - `depth`: busca sem limite inerente — 180s como rede de segurança contra
 *    engine travada (em hardware modesto, d25 pode chegar perto disso);
 *  - `time`: a engine se auto-limita a `movetimeMs`, então 3·N + 10s cobre
 *    folga de IPC/spawn.
 */
export function defaultGoTimeout(control: AnalyzeControl): number {
  return control.mode === 'depth' ? 180_000 : 3 * control.movetimeMs + 10_000
}

/** Modo de análise: por profundidade fixa ou por tempo fixo por lance. */
export type EngineMode = AnalyzeControl['mode']

/**
 * Cache de avaliações por posição, chaveado por (fen, mode, value, multipv),
 * onde `value` é `depth` (modo profundidade) ou `movetimeMs` (modo tempo).
 * `get` devolve null em caso de miss; `put` grava a avaliação alcançada.
 */
export interface PositionCache {
  get(
    fen: string,
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<RawPosition | null>
  put(
    pos: RawPosition,
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<void>
}

interface ExtractedGame {
  positionFens: string[]
  moves: PlayedMove[]
}

function extractGame(pgn: string): ExtractedGame {
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

/**
 * Handshake UCI de uma engine já spawnada: aguarda `uciok`/`readyok` e aplica
 * `setoption` de Threads, Hash e Multipv. Reutilizável entre `analyzeGame`
 * (análise nova) e a reabertura do store (só handshake, sem análise).
 *
 * Lança `Error('A engine não respondeu…')` se a engine não responder em
 * `timeoutMs` (default 10s) — sem isso, uma engine morta/travada deixaria o
 * hook pendurado para sempre.
 */
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

/**
 * Envia `cmd` pra engine e resolve quando uma linha satisfaz `done`, ou
 * rejeita após `timeoutMs` (default 10s) se a engine não responder. O listener
 * é registrado antes do send pra nunca perder a resposta.
 */
function ask(
  port: EnginePort,
  cmd: string,
  done: (line: string) => boolean,
  timeoutMs = 10_000,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const off = port.onLine((line) => {
      if (done(line)) {
        off()
        clearTimeout(timer)
        resolve()
      }
    })
    const timer = setTimeout(() => {
      off()
      reject(new Error(`A engine não respondeu a '${cmd}' em ${timeoutMs}ms.`))
    }, timeoutMs)
    void port.send(cmd)
  })
}

function uciToSan(fen: string, uci: string): string | null {
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

async function evalPosition(
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
    await ask(port, goCmd, (line) => {
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
    }, goTimeoutMs)
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

function terminalCp(fen: string): number | null {
  try {
    const c = new Chess(fen)
    if (c.isCheckmate()) return -100000
    if (c.isGameOver()) return 0
    return null
  } catch {
    return null
  }
}

/**
 * Aciona o engine posição a posição e devolve a revisão completa.
 * `port` abstrai o processo UCI (sidecar Tauri ou engine falso em testes).
 * `control` define como a engine busca cada posição: profundidade fixa
 * (`go depth N`) ou tempo fixo (`go movetime N`), este último no estilo
 * "Maximum Time" do chess.com.
 * `multipv` define quantas linhas candidatas o engine retorna por lance.
 * `opts.threads` / `opts.hashMb` dimensionam a engine (Threads/Hash) para o uso
 * ideal de CPU/RAM. Omitir mantém os defaults do Stockfish.
 * Posições terminais (xeque-mate/afogamento) são resolvidas sem chamar a engine.
 */
export async function analyzeGame(
  pgn: string,
  control: AnalyzeControl,
  port: EnginePort,
  multipv = 1,
  opts: {
    threads?: number
    hashMb?: number
    cache?: PositionCache
    /** Quando true, não envia `quit` ao final — a engine fica viva para refino ao vivo. */
    keepAlive?: boolean
    /** Override do timeout do `go` por posição (default via `defaultGoTimeout`). */
    goTimeoutMs?: number
  } = {},
): Promise<ReviewResult> {
  const { positionFens, moves } = extractGame(pgn)
  const keyValue = controlKeyValue(control)
  const goTimeoutMs = opts.goTimeoutMs ?? defaultGoTimeout(control)

  await configureEngine(port, {
    threads: opts.threads,
    hashMb: opts.hashMb,
    multipv,
  })

  const raw: RawPosition[] = []
  for (let i = 0; i < positionFens.length; i++) {
    const fen = positionFens[i]
    const term = terminalCp(fen)
    let pos: RawPosition
    if (term !== null) {
      pos = {
        fen,
        cp: term,
        depth: 0,
        pv: [],
        lines: [{ multipv: 1, cp: term, pv: [] }],
      }
    } else {
      const cached =
        (await opts.cache?.get(fen, control.mode, keyValue, multipv)) ?? null
      if (cached) {
        pos = cached
      } else {
        pos = await evalPosition(port, fen, control, goTimeoutMs)
        for (const l of pos.lines ?? []) {
          l.san = l.pv[0] ? uciToSan(pos.fen, l.pv[0]) : null
        }
        await opts.cache?.put(pos, control.mode, keyValue, multipv)
      }
    }
    raw.push(pos)
  }
  if (!opts.keepAlive) await port.send('quit')

  const opening = await lookupOpening(moves.map((m) => m.san))
  const book: BookInfo | undefined = opening
    ? { maxPly: opening.moves.length, eco: opening }
    : undefined

  return buildReview({ startFen: positionFens[0], moves }, raw, book)
}
