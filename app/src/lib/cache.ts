import { invoke } from '@tauri-apps/api/core'
import type { PositionCache, RawLine } from './analyze'

interface CachedPositionDto {
  cp: number
  linesJson: string
}

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
  }
}
