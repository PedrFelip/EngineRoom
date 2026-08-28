/**
 * Sessão de revisão: orquestração de I/O de uma partida — boot da engine
 * (com sizing best-effort), análise nova ou reabertura do store e persistência.
 * Estado de navegação mora no `ReviewStore` injetado; aqui é só sequenciamento
 * sobre o seam `Backend`, testável com fakes (mesma disciplina do `EnginePort`).
 */

import type { ReviewConfig, ReviewResult } from '../types'
import { adaptiveProfileForKind } from './adaptive-analysis'
import { analyzeGame, analyzeGameAdaptive } from './analyze'
import type { Backend } from './backend'
import type { ReviewStore } from './review-store'
import { recommendedHashMb } from './system'

export interface ReviewSessionState {
  status: 'running' | 'done' | 'error'
  error: string | null
}

export interface ReviewSessionOpts {
  config: ReviewConfig
  /** Caminho custom da engine (settings); undefined usa o sidecar. */
  enginePath: string
  backend: Backend
  store: ReviewStore
  onStateChange(state: ReviewSessionState): void
  /** winPcts parciais (POV brancas) por posição analisada — crus, sem rAF. */
  onProgress(winPcts: number[]): void
}

export interface ReviewSession {
  start(): Promise<void>
  /** Aborta tudo: engine, listeners. Assíncrono por dentro. */
  dispose(): void
}

export function createReviewSession(opts: ReviewSessionOpts): ReviewSession {
  const { config, backend, store } = opts
  let cancelled = false
  let port: Awaited<ReturnType<Backend['createEnginePort']>> = null

  const notify = (state: ReviewSessionState) => {
    if (!cancelled) opts.onStateChange(state)
  }

  async function start(): Promise<void> {
    try {
      // Reabertura instantânea: instala o resultado antes de qualquer await —
      // a tela não espera a engine subir. Sem variações exploratórias, não há
      // refino ao vivo — engine não precisa subir para initialResult.
      if (config.initialResult) {
        store.setResult(config.initialResult)
        notify({ status: 'done', error: null })
        return
      }

      port = await backend.createEnginePort(
        opts.enginePath || undefined,
        () => cancelled,
      )
      if (!port) return
      if (cancelled) {
        await port.dispose().catch(() => {})
        port = null
        return
      }

      // Dimensiona Threads/Hash (best-effort: falha → defaults do Stockfish).
      let sizing: { threads?: number; hashMb?: number } = {}
      try {
        const r = await backend.getSystemResources()
        sizing = { threads: r.threads, hashMb: recommendedHashMb(r.memory_mb) }
      } catch {
        /* fallback: defaults */
      }

      const analysisOpts = {
        ...sizing,
        cache: backend.createPositionCache(),
        keepAlive: false,
        onProgress: (wp: number[]) => {
          if (!cancelled) opts.onProgress(wp)
        },
      }
      const profile = adaptiveProfileForKind(config.analysisKind)
      let review: ReviewResult
      if (profile) {
        review = await analyzeGameAdaptive(
          config.pgn,
          profile.id,
          port,
          analysisOpts,
        )
      } else {
        const control =
          config.mode === 'time'
            ? {
                mode: 'time' as const,
                movetimeMs: config.movetimeMs ?? 5000,
              }
            : { mode: 'depth' as const, depth: config.engine.depth }
        review = await analyzeGame(
          config.pgn,
          control,
          port,
          config.lines,
          analysisOpts,
        )
      }
      if (cancelled) return
      store.setResult(review)
      notify({ status: 'done', error: null })
      void backend
        .saveReview(config, review)
        .catch((e) => console.warn('Falha ao salvar a partida no store:', e))
    } catch (e) {
      notify({
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
      })
    } finally {
      // Engine já recebeu `quit` via analyzeGame (keepAlive=false). Dispõe o
      // handle para não vazar processo/file descriptor até o unmount.
      if (port) {
        const p = port
        port = null
        void p.dispose().catch(() => {})
      }
    }
  }

  return {
    start,
    dispose() {
      cancelled = true
      const p = port
      port = null
      void p?.dispose().catch(() => {})
    },
  }
}
