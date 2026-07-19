/**
 * Live refinement of a position: keeps one or two engines running `go infinite`
 * over the current FEN and emits progressively deeper RawPositions via
 * `onMerge`.
 *
 * Two-engine model (per the project's multi-modal design):
 *  - `deep`: focused engine (e.g. Stockfish 18) running with the review's
 *    original multipv, refining the principal lines ever deeper.
 *  - `wide`: lighter engine (e.g. Stockfish 17) running with a higher multipv
 *    to surface more candidate variations. May be `null` (toggle off / idle).
 *
 * Per slot (multipv index), the merge picks whichever engine reached the
 * greater depth, so the UI always shows the most refined line available.
 *
 * Pure over `EnginePort` (the test seam) — never touches the Tauri adapter or
 * the real Stockfish directly.
 */
import type { EnginePort, PositionCache, RawLine, RawPosition } from './analyze'
import type { InfoScore } from './uci'
import { parseInfo, scoreToCp } from './uci'

export interface LiveEvalCallbacks {
  onMerge(pos: RawPosition): void
}

export interface LiveEvalOptions {
  /** Optional cache for the infinite-mode result. Saves are throttled. */
  cache?: PositionCache
}

const CACHE_THROTTLE_DEPTH = 5

export interface LiveEvalSession {
  /** Sends `position fen <current>` followed by `go infinite` on every port. */
  start(): Promise<void>
  /**
   * Switches to a new position: stops any in-flight search, clears per-slot
   * state, and starts fresh `go infinite` on the new fen.
   */
  setFen(fen: string): Promise<void>
  /**
   * Toggles the wide engine on/off. When off, the wide engine is parked via
   * UCI `stop` (stays alive but idle) and excluded from merges; when on, it's
   * pointed at the current fen and resumes `go infinite`.
   */
  setWideEnabled(enabled: boolean): Promise<void>
  /**
   * Reapplies Threads and Hash on the heavy engine only (the wide engine uses
   * its own fixed sizing). Stops the current search first because Stockfish
   * rejects `setoption Hash` mid-search.
   */
  applyHeavyResources(threads: number, hashMb: number): Promise<void>
  /**
   * Stops any in-flight search on every active engine via UCI `stop`. Engines
   * stay alive (idle); call this before dispose or when parking the session.
   */
  stop(): Promise<void>
}

interface Slot {
  depth: number
  score: InfoScore
  pv: string[]
}

type SlotMap = Map<number, Slot>

function ingest(line: string, slots: SlotMap): boolean {
  const info = parseInfo(line)
  if (!info?.score) return false
  const idx = info.multipv ?? 1
  const prev = slots.get(idx)
  if (prev && (info.depth ?? 0) < prev.depth) return false
  slots.set(idx, {
    depth: info.depth ?? 0,
    score: info.score,
    pv: info.pv ?? [],
  })
  return true
}

/** Picks, per slot, the entry with the greater depth. */
function mergeLines(deep: SlotMap, wide: SlotMap | null): RawLine[] {
  const indices = new Set<number>([...deep.keys(), ...(wide?.keys() ?? [])])
  return [...indices]
    .sort((a, b) => a - b)
    .map((idx) => {
      const d = deep.get(idx)
      const w = wide?.get(idx)
      const pick = !w || (d && d.depth >= w.depth) ? d : w
      const s = pick ?? d ?? w
      return {
        multipv: idx,
        cp: s ? (scoreToCp(s.score) ?? 0) : 0,
        pv: s?.pv ?? [],
      }
    })
}

function buildPosition(
  fen: string,
  deep: SlotMap,
  wide: SlotMap | null,
): RawPosition {
  const lines = mergeLines(deep, wide)
  const primary = lines.find((l) => l.multipv === 1) ?? lines[0]
  return {
    fen,
    cp: primary?.cp ?? 0,
    depth: deep.get(1)?.depth ?? wide?.get(1)?.depth ?? 0,
    pv: primary?.pv ?? [],
    lines,
  }
}

export function createLiveEvalSession(
  ports: { deep: EnginePort; wide: EnginePort | null },
  initial: { fen: string },
  cb: LiveEvalCallbacks,
  opts: LiveEvalOptions = {},
): LiveEvalSession {
  let curFen = initial.fen
  let wideActive = ports.wide !== null
  let lastSavedDepth: number | null = null
  const deepSlots: SlotMap = new Map()
  const wideSlots: SlotMap = new Map()

  const emit = () => {
    const pos = buildPosition(curFen, deepSlots, wideActive ? wideSlots : null)
    cb.onMerge(pos)
    maybePersist(pos)
  }

  const maybePersist = (pos: RawPosition) => {
    if (!opts.cache?.putInfinite) return
    if (
      lastSavedDepth !== null &&
      pos.depth < lastSavedDepth + CACHE_THROTTLE_DEPTH
    )
      return
    lastSavedDepth = pos.depth
    void opts.cache.putInfinite(pos).catch(() => {
      /* best-effort: cache write failures don't break the live refinement */
    })
  }

  ports.deep.onLine((line) => {
    if (ingest(line, deepSlots)) emit()
  })
  if (ports.wide) {
    ports.wide.onLine((line) => {
      if (!wideActive) return
      if (ingest(line, wideSlots)) emit()
    })
  }

  return {
    async start() {
      await ports.deep.send(`position fen ${curFen}`)
      await ports.deep.send('go infinite')
      if (ports.wide && wideActive) {
        await ports.wide.send(`position fen ${curFen}`)
        await ports.wide.send('go infinite')
      }
    },
    async setFen(fen: string) {
      curFen = fen
      deepSlots.clear()
      wideSlots.clear()
      lastSavedDepth = null
      await ports.deep.send('stop')
      await ports.deep.send(`position fen ${curFen}`)
      await ports.deep.send('go infinite')
      if (ports.wide) {
        await ports.wide.send('stop')
        if (wideActive) {
          await ports.wide.send(`position fen ${curFen}`)
          await ports.wide.send('go infinite')
        }
      }
    },
    async setWideEnabled(enabled: boolean) {
      if (!ports.wide || enabled === wideActive) return
      wideActive = enabled
      if (!enabled) {
        await ports.wide.send('stop')
        wideSlots.clear()
        emit()
      } else {
        wideSlots.clear()
        await ports.wide.send(`position fen ${curFen}`)
        await ports.wide.send('go infinite')
      }
    },
    async applyHeavyResources(threads: number, hashMb: number) {
      await ports.deep.send('stop')
      await ports.deep.send(`setoption name Threads value ${threads}`)
      await ports.deep.send(`setoption name Hash value ${hashMb}`)
      await ports.deep.send(`position fen ${curFen}`)
      await ports.deep.send('go infinite')
    },
    async stop() {
      await ports.deep.send('stop')
      if (ports.wide && wideActive) {
        await ports.wide.send('stop')
      }
    },
  }
}
