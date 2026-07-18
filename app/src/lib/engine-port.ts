import type { EnginePort } from './analyze'
import { engineSend, engineStart, engineStop, onEngineLine } from './engine'

export interface TauriEnginePort extends EnginePort {
  dispose: () => Promise<void>
}

/**
 * Cria um EnginePort sobre o processo Stockfish do Tauri.
 *
 * `isCancelled` é consultado entre cada etapa (stop → start → listen) para que
 * um efeito abortado (ex.: StrictMode em dev, que monta→desmonta→monta) saia
 * antes de spawnar a engine — evitando "engine já está em execução".
 * O listener de linhas é registrado antes de qualquer send para nunca perder
 * respostas (mesmo uciok/readyok). Devolve null se abortado.
 */
export async function createTauriEnginePort(
  path: string | undefined,
  isCancelled: () => boolean,
): Promise<TauriEnginePort | null> {
  await engineStop().catch(() => {})
  if (isCancelled()) return null
  await engineStart(path)
  if (isCancelled()) {
    await engineStop().catch(() => {})
    return null
  }
  const handlers = new Set<(line: string) => void>()
  const unlisten = await onEngineLine((line) => {
    handlers.forEach((h) => h(line))
  })
  if (isCancelled()) {
    unlisten()
    await engineStop().catch(() => {})
    return null
  }
  return {
    send: (cmd: string) => engineSend(cmd),
    onLine(handler: (line: string) => void) {
      handlers.add(handler)
      return () => {
        handlers.delete(handler)
      }
    },
    async dispose() {
      unlisten()
      await engineStop().catch(() => {})
    },
  }
}
