import { invoke } from '@tauri-apps/api/core'
import type { PositionCache, RawLine, RawPosition } from './analyze'

interface CachedPositionDto {
  cp: number
  linesJson: string
}

interface InfinitePayload {
  depth: number
  lines: RawLine[]
}

const INFINITE_MODE = 'infinite'
const INFINITE_SENTINEL_DEPTH = 0
const INFINITE_SENTINEL_MULTIPV = 1

/**
 * PositionCache persistido no SQLite do lado Rust (comandos cache_get/cache_put).
 * Falhas de I/O propagam como erro de análise — o cache é caminho crítico,
 * não best-effort.
 */
export function createTauriPositionCache(): PositionCache {
  return {
    async get(fen, mode, value, multipv) {
      const hit = await invoke<CachedPositionDto | null>('cache_get', {
        fen,
        mode,
        depth: value,
        multipv,
      })
      if (!hit) return null
      const lines = JSON.parse(hit.linesJson) as RawLine[]
      const principal = lines.find((l) => l.multipv === 1) ?? lines[0]
      return { fen, cp: hit.cp, depth: value, pv: principal?.pv ?? [], lines }
    },
    async put(pos, mode, value, multipv) {
      await invoke('cache_put', {
        fen: pos.fen,
        mode,
        depth: value,
        multipv,
        cp: pos.cp,
        linesJson: JSON.stringify(pos.lines ?? []),
      })
    },
    async getInfinite(fen) {
      const hit = await invoke<CachedPositionDto | null>('cache_get', {
        fen,
        mode: INFINITE_MODE,
        depth: INFINITE_SENTINEL_DEPTH,
        multipv: INFINITE_SENTINEL_MULTIPV,
      })
      if (!hit) return null
      const payload = JSON.parse(hit.linesJson) as InfinitePayload
      const principal =
        payload.lines.find((l) => l.multipv === 1) ?? payload.lines[0]
      return {
        fen,
        cp: hit.cp,
        depth: payload.depth,
        pv: principal?.pv ?? [],
        lines: payload.lines,
      }
    },
    async putInfinite(pos) {
      const payload: InfinitePayload = {
        depth: pos.depth,
        lines: pos.lines ?? [],
      }
      await invoke('cache_put', {
        fen: pos.fen,
        mode: INFINITE_MODE,
        depth: INFINITE_SENTINEL_DEPTH,
        multipv: INFINITE_SENTINEL_MULTIPV,
        cp: pos.cp,
        linesJson: JSON.stringify(payload),
      })
    },
  }
}

export type { RawPosition }
