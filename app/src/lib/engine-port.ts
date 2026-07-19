import type { EnginePort } from './analyze'
import { engineSend, engineStart, engineStop, onEngineLine } from './engine'

export interface TauriEnginePort extends EnginePort {
  /** id under which this port's engine is registered with the Rust layer. */
  readonly id: string
  dispose: () => Promise<void>
}

/**
 * Cria um EnginePort sobre um processo Stockfish registrado sob `id`. Cada
 * porta é dona do seu id — múltiplas portas podem coexistir, cada uma ouvindo
 * só as linhas da sua engine.
 *
 * `isCancelled` é consultado entre cada etapa (stop → start → listen) para que
 * um efeito abortado (ex.: StrictMode em dev, que monta→desmonta→monta) saia
 * antes de spawnar a engine — evitando "engine já está em execução".
 * O listener de linhas é registrado antes de qualquer send para nunca perder
 * respostas (mesmo uciok/readyok). Devolve null se abortado.
 */
export async function createTauriEnginePort(
  id: string,
  path: string | undefined,
  isCancelled: () => boolean,
): Promise<TauriEnginePort | null> {
  await engineStop(id).catch(() => {})
  if (isCancelled()) return null
  await engineStart(id, path)
  if (isCancelled()) {
    await engineStop(id).catch(() => {})
    return null
  }
  const handlers = new Set<(line: string) => void>()
  const unlisten = await onEngineLine((lineId, line) => {
    if (lineId !== id) return
    handlers.forEach((h) => {
      h(line)
    })
  })
  if (isCancelled()) {
    unlisten()
    await engineStop(id).catch(() => {})
    return null
  }
  return {
    id,
    send: (cmd: string) => engineSend(id, cmd),
    onLine(handler: (line: string) => void) {
      handlers.add(handler)
      return () => {
        handlers.delete(handler)
      }
    },
    async dispose() {
      unlisten()
      await engineStop(id).catch(() => {})
    },
  }
}
