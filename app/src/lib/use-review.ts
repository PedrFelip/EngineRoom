import { useCallback, useEffect, useState } from 'react'
import type { ReviewConfig, ReviewResult } from '../types'
import { analyzeGame } from './analyze'
import { createTauriPositionCache } from './cache'
import { createTauriEnginePort } from './engine-port'
import { saveReview } from './games'
import { useSettings } from './settings-context'
import { getSystemResources, recommendedHashMb } from './system'

export type ReviewStatus = 'running' | 'done' | 'error'

export interface UseReview {
  result: ReviewResult | null
  status: ReviewStatus
  error: string | null
  currentPly: number
  orientation: 'white' | 'black'
  goTo: (ply: number) => void
  next: () => void
  prev: () => void
  first: () => void
  last: () => void
  flip: () => void
}

export function useReview(config: ReviewConfig): UseReview {
  const { settings } = useSettings()
  const [result, setResult] = useState<ReviewResult | null>(null)
  const [status, setStatus] = useState<ReviewStatus>('running')
  const [error, setError] = useState<string | null>(null)
  const [currentPly, setCurrentPly] = useState(0)
  const [orientation, setOrientation] = useState<'white' | 'black'>('white')

  useEffect(() => {
    // Partida vinda do store: reabertura instantânea, sem engine e sem regravar.
    if (config.initialResult) {
      setResult(config.initialResult)
      setCurrentPly(config.initialResult.moves.length)
      setStatus('done')
      return
    }

    let cancelled = false
    let port: { dispose: () => Promise<void> } | null = null

    ;(async () => {
      try {
        const p = await createTauriEnginePort(
          settings.enginePath || undefined,
          () => cancelled,
        )
        if (cancelled || !p) return
        port = p

        // Dimensiona a engine pra usar o máximo de CPU/RAM (Threads + Hash).
        // Best-effort: se a detecção falhar, segue com os defaults do Stockfish.
        let sizing: { threads?: number; hashMb?: number } = {}
        try {
          const r = await getSystemResources()
          sizing = {
            threads: r.threads,
            hashMb: recommendedHashMb(r.memory_mb),
          }
        } catch {
          /* fallback: defaults */
        }

        const review = await analyzeGame(
          config.pgn,
          config.engine.depth,
          p,
          config.lines,
          { ...sizing, cache: createTauriPositionCache() },
        )
        if (cancelled) return
        setResult(review)
        setCurrentPly(review.moves.length)
        setStatus('done')
        void saveReview(config, review).catch((e) =>
          console.warn('Falha ao salvar a partida no store:', e),
        )
      } catch (e) {
        if (cancelled) return
        setError(e instanceof Error ? e.message : String(e))
        setStatus('error')
      }
    })()

    return () => {
      cancelled = true
      void port?.dispose()
    }
  }, [config, settings.enginePath])

  const total = result?.moves.length ?? 0

  const goTo = useCallback(
    (ply: number) => setCurrentPly(Math.max(0, Math.min(total, ply))),
    [total],
  )
  const next = useCallback(
    () => setCurrentPly((p) => Math.min(total, p + 1)),
    [total],
  )
  const prev = useCallback(() => setCurrentPly((p) => Math.max(0, p - 1)), [])
  const first = useCallback(() => setCurrentPly(0), [])
  const last = useCallback(() => setCurrentPly(total), [total])
  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )

  return {
    result,
    status,
    error,
    currentPly,
    orientation,
    goTo,
    next,
    prev,
    first,
    last,
    flip,
  }
}
