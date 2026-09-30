import { Effect } from 'effect'
import { errorMessage } from '../../effect/errors'
import { ask } from './analysis/engine-analysis'
import { createTauriEnginePort } from './engine-port'
import { isUciOk, parseIdName } from './uci'

export { ENGINE_EXIT_EVENT, ENGINE_LINE_EVENT } from './engine-port'
export interface ProbeResult {
  ok: boolean
  name: string | null
  error?: string
}
export interface ProbeOptions {
  timeoutMs?: number
}

/** Uses the same exclusive scoped engine as reviews. */
export function probeEngine({
  timeoutMs = 8000,
}: ProbeOptions = {}): Effect.Effect<ProbeResult> {
  return Effect.scoped(
    Effect.gen(function* () {
      const port = yield* createTauriEnginePort
      let name: string | null = null
      yield* ask(
        port,
        'uci',
        (line) => {
          name = parseIdName(line) ?? name
          return isUciOk(line)
        },
        timeoutMs,
      )
      return { ok: true, name }
    }),
  ).pipe(
    Effect.catchAll((error) =>
      Effect.succeed({
        ok: false,
        name: null,
        error: errorMessage(error),
      }),
    ),
  )
}
