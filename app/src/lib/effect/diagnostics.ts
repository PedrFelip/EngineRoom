import { Effect } from 'effect'

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
    Effect.catchCause((cause) => warn(operation, cause)),
    Effect.asVoid,
  )
}
