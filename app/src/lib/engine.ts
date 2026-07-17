import { invoke } from "@tauri-apps/api/core";
import { type UnlistenFn, listen } from "@tauri-apps/api/event";
import { isUciOk, parseIdName } from "./uci";

export const ENGINE_LINE_EVENT = "engine://line";

/** Spawns the engine. Pass a path to use a custom Stockfish; omit to use the embedded sidecar. */
export function engineStart(path?: string): Promise<void> {
  return invoke("engine_spawn", { path: path?.trim() ? path.trim() : null });
}

/** Sends a single UCI command (no trailing newline needed). */
export function engineSend(line: string): Promise<void> {
  return invoke("engine_send", { line });
}

/** Stops and disposes the current engine process. */
export function engineStop(): Promise<void> {
  return invoke("engine_stop");
}

/** Subscribes to every UCI line the engine prints to stdout. */
export function onEngineLine(cb: (line: string) => void): Promise<UnlistenFn> {
  return listen<string>(ENGINE_LINE_EVENT, (e) => cb(e.payload));
}

export interface ProbeResult {
  ok: boolean;
  name: string | null;
  error?: string;
}

export interface ProbeOptions {
  timeoutMs?: number;
}

/**
 * Spawns the engine, sends `uci`, waits for `uciok`, then stops it.
 * Used by the Settings screen to verify the embedded sidecar or a custom path.
 */
export async function probeEngine(
  path?: string,
  { timeoutMs = 8000 }: ProbeOptions = {},
): Promise<ProbeResult> {
  let unlisten: UnlistenFn | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;

  try {
    await engineStop().catch(() => {});
    await engineStart(path);

    let name: string | null = null;
    let resolveResult!: (r: ProbeResult) => void;
    const done = new Promise<ProbeResult>((resolve) => {
      resolveResult = resolve;
    });

    // Register the listener BEFORE sending `uci` so we never miss the reply.
    unlisten = await onEngineLine((line) => {
      const parsedName = parseIdName(line);
      if (parsedName) name = parsedName;
      if (isUciOk(line)) resolveResult({ ok: true, name });
    });

    timer = setTimeout(
      () => resolveResult({ ok: false, name, error: "Tempo esgotado aguardando a engine responder (uciok)." }),
      timeoutMs,
    );

    await engineSend("uci");

    return await done;
  } catch (e) {
    return { ok: false, name: null, error: e instanceof Error ? e.message : String(e) };
  } finally {
    if (timer) clearTimeout(timer);
    if (unlisten) unlisten();
    await engineStop().catch(() => {});
  }
}
