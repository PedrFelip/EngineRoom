import { Effect, Metric } from 'effect'

/** Low-cardinality, in-process metrics. No exporter or remote telemetry. */
export const metrics = {
  positions: Metric.counter('analysis.positions'),
  cacheHits: Metric.counter('analysis.cache_hits'),
  cacheMisses: Metric.counter('analysis.cache_misses'),
  timeouts: Metric.counter('engine.timeouts'),
  retries: Metric.counter('engine.retries'),
  interruptions: Metric.counter('analysis.interruptions'),
  persistenceFailures: Metric.counter('storage.failures'),
  duration: Metric.timer('engine.search_duration'),
}

/** A local snapshot for diagnostics/tests; never includes PGN or FEN labels. */
export const diagnosticsSnapshot = Effect.all({
  positions: Metric.value(metrics.positions),
  cacheHits: Metric.value(metrics.cacheHits),
  cacheMisses: Metric.value(metrics.cacheMisses),
  timeouts: Metric.value(metrics.timeouts),
  retries: Metric.value(metrics.retries),
  interruptions: Metric.value(metrics.interruptions),
  persistenceFailures: Metric.value(metrics.persistenceFailures),
  searchDuration: Metric.value(metrics.duration),
})

export function warn(operation: string, cause: unknown) {
  return Effect.logWarning('Falha em operação best-effort').pipe(
    Effect.annotateLogs({ operation, cause }),
  )
}

export function bestEffort<A, E, R>(
  effect: Effect.Effect<A, E, R>,
  operation: string,
) {
  return effect.pipe(
    Effect.catchAllCause((cause) => warn(operation, cause)),
    Effect.asVoid,
  )
}
