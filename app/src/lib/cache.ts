import { invoke } from '@tauri-apps/api/core'
import type { PositionCache, RawLine, RawPosition } from './analyze'

export interface CachedPositionDto {
  cp: number
  linesJson: string
  reachedDepth: number
}

/**
 * Reconstrói um `RawPosition` a partir do DTO do Rust, fatiando as linhas ao
 * multipv pedido e usando o **depth real atingido** (da pv-1 ou `reachedDepth`),
 * nunca o escalar do pedido. Isso corrige o bug onde um hit em modo tempo
 * reportava `depth = movetimeMs` (ex.: 5000) em vez da profundidade real.
 */
export function shapeCachedPosition(
  hit: CachedPositionDto,
  fen: string,
  requestedMultipv: number,
): RawPosition {
  const allLines = JSON.parse(hit.linesJson) as RawLine[]
  const lines = allLines.slice(0, requestedMultipv)
  const principal = lines.find((l) => l.multipv === 1) ?? lines[0]
  return {
    fen,
    cp: hit.cp,
    depth: principal?.depth ?? hit.reachedDepth ?? 0,
    pv: principal?.pv ?? [],
    lines,
  }
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
      return shapeCachedPosition(hit, fen, multipv)
    },
    async put(pos, mode, value, multipv) {
      await invoke('cache_put', {
        fen: pos.fen,
        mode,
        depth: value,
        multipv,
        reachedDepth: pos.depth,
        cp: pos.cp,
        linesJson: JSON.stringify(pos.lines ?? []),
      })
    },
  }
}
