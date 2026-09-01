/**
 * Seam `Backend`: todo o I/O do desktop (engine, cache, store de partidas,
 * recursos do sistema) por trás de uma interface injetável. Dois adapters a
 * tornam real — o Tauri em produção e fakes nos testes de `review-session`,
 * do mesmo jeito que `EnginePort`/`PositionCache` fazem pela análise.
 */

import type { ReviewConfig, ReviewResult } from '../types'
import type { EnginePort, PositionCache } from './analyze'
import { createTauriPositionCache } from './cache'
import { createTauriEnginePort } from './engine-port'
import { saveReview } from './games'
import type { SystemResources } from './system'
import { getSystemResources } from './system'

/** Porta de engine com ciclo de vida próprio (encerra o processo no dispose). */
export type EnginePortHandle = EnginePort & { dispose(): Promise<void> }

export interface Backend {
  /**
   * Sobe a engine sidecar. Devolve null se `isCancelled`
   * virar true durante o boot — sem processos órfãos. `isCancelled` é
   * consultado entre cada etapa (stop → start → listen) para efeitos
   * abortados (ex.: StrictMode) saírem antes de spawnar.
   */
  createEnginePort(isCancelled: () => boolean): Promise<EnginePortHandle | null>
  /** Recursos da máquina (Threads/Hash); falha tratada como best-effort. */
  getSystemResources(): Promise<SystemResources>
  /** Cache de posições persistido (SQLite no Rust). */
  createPositionCache(): PositionCache
  /** Persiste a revisão analisada no store de partidas (upsert). */
  saveReview(config: ReviewConfig, result: ReviewResult): Promise<number>
}

/** Adapter Tauri: compõe os wrappers IPC já existentes, um por operação. */
export function createTauriBackend(): Backend {
  return {
    createEnginePort: (isCancelled) => createTauriEnginePort(isCancelled),
    getSystemResources: () => getSystemResources(),
    createPositionCache: () => createTauriPositionCache(),
    saveReview: (config, result) => saveReview(config, result),
  }
}
