import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { isUciOk, parseIdName } from './uci'

export const ENGINE_LINE_EVENT = 'engine://line'

export interface EngineLinePayload {
  id: string
  line: string
}

/**
 * Spawns an engine registered under `id`. Pass `sidecar` to use a bundled
 * sidecar by basename (e.g. "stockfish" or "stockfish-lite"); pass `path` to
 * use a custom Stockfish binary on the host filesystem. `path` wins over
 * `sidecar` when both are set; the default sidecar is "stockfish".
 *
 * Multiple engines can coexist as long as each one uses a distinct id.
 */
export function engineStart(
  id: string,
  sidecar?: string,
  path?: string,
): Promise<void> {
  return invoke('engine_spawn', {
    id,
    sidecar: sidecar ?? null,
    path: path?.trim() ? path.trim() : null,
  })
}

/** sends a single UCI command (no trailing newline) to the engine named `id`. */
export function engineSend(id: string, line: string): Promise<void> {
  return invoke('engine_send', { id, line })
}

/** stops and disposes the engine registered as `id`. */
export function engineStop(id: string): Promise<void> {
  return invoke('engine_stop', { id })
}

/**
 * subscribes to every UCI line the engines print to stdout. The callback
 * receives the engine id alongside the line so multiple engines can be
 * distinguished.
 */
export function onEngineLine(
  cb: (id: string, line: string) => void,
): Promise<UnlistenFn> {
  return listen<EngineLinePayload>(ENGINE_LINE_EVENT, (e) =>
    cb(e.payload.id, e.payload.line),
  )
}

export interface ProbeResult {
  ok: boolean
  name: string | null
  error?: string
}

export interface ProbeOptions {
  timeoutMs?: number
}

/**
 * Spawns the engine, sends `uci`, waits for `uciok`, then stops it.
 * Used by the Settings screen to verify the embedded sidecar or a custom path.
 *
 * Uses the reserved id `"probe"` so it never collides with review sessions.
 */
export async function probeEngine(
  path?: string,
  { timeoutMs = 8000 }: ProbeOptions = {},
): Promise<ProbeResult> {
  const probeId = 'probe'
  let unlisten: UnlistenFn | undefined
  let timer: ReturnType<typeof setTimeout> | undefined

  try {
    await engineStop(probeId).catch(() => {})
    await engineStart(probeId, undefined, path)

    let name: string | null = null
    let resolveResult!: (r: ProbeResult) => void
    const done = new Promise<ProbeResult>((resolve) => {
      resolveResult = resolve
    })

    // Register the listener BEFORE sending `uci` so we never miss the reply.
    unlisten = await onEngineLine((id, line) => {
      if (id !== probeId) return
      const parsedName = parseIdName(line)
      if (parsedName) name = parsedName
      if (isUciOk(line)) resolveResult({ ok: true, name })
    })

    timer = setTimeout(
      () =>
        resolveResult({
          ok: false,
          name,
          error: 'Tempo esgotado aguardando a engine responder (uciok).',
        }),
      timeoutMs,
    )

    await engineSend(probeId, 'uci')

    return await done
  } catch (e) {
    return {
      ok: false,
      name: null,
      error: e instanceof Error ? e.message : String(e),
    }
  } finally {
    if (timer) clearTimeout(timer)
    if (unlisten) unlisten()
    await engineStop('probe').catch(() => {})
  }
}
