import { Effect, Either, Metric, Schema } from 'effect'
import type { PositionCache, RawPosition } from './analyze'
import { metrics } from './effect/diagnostics'
import { CacheError, errorMessage } from './effect/errors'
import { ipc } from './effect/ipc'
import { CachedPositionSchema, CacheLinesSchema } from './effect/schemas'

export interface CachedPositionDto {
  cp: number
  linesJson: string
  reachedDepth: number
}

/** Validate once on entry. Corrupt/legacy-incompatible rows are cache misses. */
export function shapeCachedPosition(
  hit: unknown,
  fen: string,
  requestedMultipv: number,
): RawPosition | null {
  const dto = Schema.decodeUnknownEither(CachedPositionSchema)(hit)
  if (Either.isLeft(dto)) return null
  const decoded = Schema.decodeUnknownEither(CacheLinesSchema)(
    dto.right.linesJson,
  )
  if (Either.isLeft(decoded) || decoded.right.length === 0) return null
  const lines = decoded.right.slice(0, requestedMultipv)
  const principal = lines.find((l) => l.multipv === 1) ?? lines[0]
  return {
    fen,
    cp: dto.right.cp,
    depth: principal?.depth ?? dto.right.reachedDepth,
    pv: principal?.pv ?? [],
    lines,
  }
}
function cacheIpc(operation: string, args: Record<string, unknown>) {
  return ipc(
    operation,
    args,
    (cause) =>
      new CacheError({
        operation,
        cause,
        message: errorMessage(cause),
      }),
  )
}
function recordHit(pos: RawPosition | null) {
  return Metric.increment(pos ? metrics.cacheHits : metrics.cacheMisses)
}

export function createTauriPositionCache(): PositionCache {
  return {
    get: (fen, mode, value, multipv) =>
      cacheIpc('cache_get', {
        fen,
        mode,
        depth: value,
        multipv,
      }).pipe(
        Effect.map((hit) => shapeCachedPosition(hit, fen, multipv)),
        Effect.tap(recordHit),
      ),
    put: (pos, mode, value, multipv) =>
      cacheIpc('cache_put', {
        fen: pos.fen,
        mode,
        depth: value,
        multipv,
        reachedDepth: pos.depth,
        cp: pos.cp,
        linesJson: JSON.stringify(pos.lines ?? []),
      }).pipe(Effect.asVoid),
    getBulk: (fens, mode, value, multipv) =>
      cacheIpc('cache_get_bulk', {
        fens,
        mode,
        depth: value,
        multipv,
      }).pipe(
        Effect.map((hits) =>
          fens.map((fen, i) =>
            Array.isArray(hits) && hits.length === fens.length
              ? shapeCachedPosition(hits[i], fen, multipv)
              : null,
          ),
        ),
        Effect.tap((hits) =>
          Effect.forEach(hits, recordHit, { discard: true }),
        ),
      ),
    putMany: (entries, mode, value, multipv) =>
      cacheIpc('cache_put_many', {
        entries: entries.map((e) => ({
          fen: e.fen,
          reachedDepth: e.depth,
          cp: e.cp,
          linesJson: JSON.stringify(e.lines ?? []),
        })),
        mode,
        depth: value,
        multipv,
      }).pipe(Effect.asVoid),
  }
}
