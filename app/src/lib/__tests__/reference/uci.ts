/**
 * Pure parsers for the UCI (Universal Chess Interface) protocol.
 * Kept side-effect free so they can be unit tested without an engine running.
 * Reference: https://stockfishchess.org/download/ and the engine's wiki/UCI-&-Commands.md
 */

export interface BestMove {
  from: string
  to: string
  promotion?: string
}

export interface InfoScore {
  kind: 'cp' | 'mate'
  value: number
  lowerbound?: boolean
  upperbound?: boolean
}

export interface ParsedInfo {
  depth?: number
  seldepth?: number
  multipv?: number
  nodes?: number
  nps?: number
  time?: number
  score?: InfoScore
  pv?: string[]
}

/** `id name Stockfish 18` -> `Stockfish 18`. */
export function parseIdName(line: string): string | null {
  const m = /^id\s+name\s+(.+)$/i.exec(line.trim())
  return m ? m[1].trim() : null
}

/** `id author the Stockfish developers` -> that string. */
export function parseIdAuthor(line: string): string | null {
  const m = /^id\s+author\s+(.+)$/i.exec(line.trim())
  return m ? m[1].trim() : null
}

export function isUciOk(line: string): boolean {
  return line.trim() === 'uciok'
}

export function isReadyOk(line: string): boolean {
  return line.trim() === 'readyok'
}

/** `bestmove e2e4` / `bestmove e7e8q` / `bestmove (none)` -> structured move or null. */
export function parseBestMove(line: string): BestMove | null {
  const m = /^bestmove\s+(\S+)/i.exec(line.trim())
  if (!m) return null
  const uci = m[1]
  if (uci === '(none)' || uci.length < 4) return null
  const from = uci.slice(0, 2)
  const to = uci.slice(2, 4)
  const promotion = uci.length >= 5 ? uci[4] : undefined
  return { from, to, ...(promotion ? { promotion } : {}) }
}

function num(token: string | undefined): number | undefined {
  if (token === undefined) return undefined
  const n = Number(token)
  return Number.isFinite(n) ? n : undefined
}

/**
 * Parses an `info ...` line. Handles the fields we care about for review:
 * depth, seldepth, multipv, nodes, nps, time, score (cp|mate [+ lower/upperbound]), pv.
 */
export function parseInfo(line: string): ParsedInfo | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith('info')) return null

  const tokens = trimmed.split(/\s+/)
  const out: ParsedInfo = {}

  for (let i = 1; i < tokens.length; i++) {
    switch (tokens[i]) {
      case 'depth':
        out.depth = num(tokens[++i])
        break
      case 'seldepth':
        out.seldepth = num(tokens[++i])
        break
      case 'multipv':
        out.multipv = num(tokens[++i])
        break
      case 'nodes':
        out.nodes = num(tokens[++i])
        break
      case 'nps':
        out.nps = num(tokens[++i])
        break
      case 'time':
        out.time = num(tokens[++i])
        break
      case 'score': {
        const kind = tokens[++i] === 'mate' ? 'mate' : 'cp'
        const value = num(tokens[++i]) ?? 0
        const score: InfoScore = { kind, value }
        const next = tokens[i + 1]
        if (next === 'lowerbound') {
          score.lowerbound = true
          i++
        } else if (next === 'upperbound') {
          score.upperbound = true
          i++
        }
        out.score = score
        break
      }
      case 'pv':
        out.pv = tokens.slice(i + 1)
        i = tokens.length
        break
      case 'string':
        // Free-form line; ignore the remainder.
        i = tokens.length
        break
      default:
        break
    }
  }

  return out
}

/**
 * Normalizes a score to centipawns from the side-to-move's perspective.
 * Mate values map to a large signed centipawn magnitude.
 */
export function scoreToCp(score: InfoScore | undefined): number | null {
  if (!score) return null
  if (score.kind === 'cp') return score.value
  // mate in N: convert to a big number keeping the sign.
  // mate 0 = posição de xeque-mate (lado a jogar perdeu) → magnitude máxima negativa.
  const sign = score.value > 0 ? 1 : -1
  const magnitude = Math.abs(score.value)
  return sign * (100000 - magnitude)
}
