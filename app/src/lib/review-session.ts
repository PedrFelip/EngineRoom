/**
 * Sessão de revisão: orquestração de I/O de uma partida — boot da engine
 * (com sizing best-effort), análise nova ou reabertura do store, persistência
 * e sessão de refino ao vivo de variações. Estado de navegação/variação mora
 * no `ReviewStore` injetado; aqui é só sequenciamento sobre o seam `Backend`,
 * testável com fakes (mesma disciplina do `EnginePort`).
 */

import type { ReviewConfig } from '../types'
import { analyzeGame, configureEngine } from './analyze'
import type { Backend } from './backend'
import type { ReviewStore } from './review-store'
import { recommendedHashMb } from './system'
import {
  createVariationEvalSession,
  type VariationEvalSession,
} from './variation-eval'

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
  /**
   * Reponta o refino ao vivo para o FEN exibido. Deve ser chamado a cada
   * mudança de posição em exibição — e só então (troca de FEN), para não
   * reiniciar `go infinite` quando o lance apenas recebe nota.
   */
  setDisplayedFen(fen: string): void
  /** Aborta tudo: refino, engine, listeners. Assíncrono por dentro. */
  dispose(): void
}

export function createReviewSession(opts: ReviewSessionOpts): ReviewSession {
  const { config, backend, store } = opts
  let cancelled = false
  let evalSession: VariationEvalSession | null = null
  let port: Awaited<ReturnType<Backend['createEnginePort']>> = null
  // Último FEN que o refino avalia — reprontar o mesmo FEN seria um restart
  // desnecessário (stop + go infinite) que ainda limparia o estado acumulado.
  let lastFen: string | null = null

  const notify = (state: ReviewSessionState) => {
    if (!cancelled) opts.onStateChange(state)
  }

  async function start(): Promise<void> {
    try {
      // Reabertura instantânea: instala o resultado antes de qualquer await —
      // a tela não espera a engine subir.
      if (config.initialResult) {
        store.setResult(config.initialResult)
        notify({ status: 'done', error: null })
      }

      port = await backend.createEnginePort(
        opts.enginePath || undefined,
        () => cancelled,
      )
      if (!port) return
      if (cancelled) {
        // Janela cancelado-mas-porta-viva: encerra a porta, sem órfãos.
        await port.dispose().catch(() => {})
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

      // Ramo: análise nova (analyzeGame faz handshake E analisa) vs
      // reabertura do store (só handshake — o review já temos).
      let current = config.initialResult ?? null
      if (config.initialResult) {
        await configureEngine(port, { ...sizing, multipv: config.lines })
        if (cancelled) return
      } else {
        const control =
          config.mode === 'time'
            ? {
                mode: 'time' as const,
                movetimeMs: config.movetimeMs ?? 5000,
              }
            : { mode: 'depth' as const, depth: config.engine.depth }
        // keepAlive: true → a engine permanece viva para o refino de variações.
        const review = await analyzeGame(
          config.pgn,
          control,
          port,
          config.lines,
          {
            ...sizing,
            cache: backend.createPositionCache(),
            keepAlive: true,
            onProgress: (wp) => {
              if (!cancelled) opts.onProgress(wp)
            },
          },
        )
        if (cancelled) return
        store.setResult(review)
        notify({ status: 'done', error: null })
        void backend
          .saveReview(config, review)
          .catch((e) => console.warn('Falha ao salvar a partida no store:', e))
        current = review
      }
      if (!current) return

      // === Refino de variações ===
      // Loop minimal sobre a engine já viva. Se o usuário já navegou para um
      // ply diferente do final antes da engine subir, aponta para o FEN
      // exibido — evita avaliar posição que ninguém está vendo.
      const initialFen =
        store.getDisplayedFen() ??
        current.positions[current.moves.length]?.fen ??
        current.positions[0].fen
      evalSession = createVariationEvalSession(
        port,
        { fen: initialFen, multipv: config.lines },
        {
          onMerge: (pos) => {
            const target = store.getAnalysisTarget()
            if (target.kind !== 'variation') return
            store.applyLive(target, pos)
          },
        },
      )
      lastFen = initialFen
      if (cancelled) return
      await evalSession.start()
    } catch (e) {
      notify({
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
      })
    }
  }

  return {
    start,
    setDisplayedFen(fen: string) {
      if (fen === lastFen) return
      lastFen = fen
      void evalSession?.setFen(fen).catch(() => {})
    },
    dispose() {
      cancelled = true
      const session = evalSession
      const p = port
      evalSession = null
      port = null
      void (async () => {
        try {
          if (session) await session.stop()
        } catch {
          /* ignore */
        }
        await p?.dispose().catch(() => {})
      })()
    },
  }
}
