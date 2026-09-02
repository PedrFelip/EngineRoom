/**
 * Ponte React da revisão: o estado e as transições moram no `ReviewStore`
 * (puro) e a orquestração de I/O na `ReviewSession` (seam `Backend`); este
 * hook é só cola de view — bridging de snapshot, coalescing de progresso em
 * rAF e orientação do tabuleiro.
 */

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react'
import type { ReviewConfig, ReviewResult } from '../types'
import { createTauriBackend } from './backend'
import {
  createReviewSession,
  type ReviewProgress,
  type ReviewSession,
  type ReviewSessionState,
} from './review-session'
import { createReviewStore, type ReviewStore } from './review-store'

export interface UseReview {
  result: ReviewResult | null
  status: ReviewSessionState['status']
  error: string | null
  /** winPcts parciais (POV brancas) que alimentam o gráfico durante o loading. */
  partialWinPcts: readonly number[]
  progress: ReviewProgress
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
  const storeRef = useRef<ReviewStore | null>(null)
  if (!storeRef.current) storeRef.current = createReviewStore()
  const store = storeRef.current
  const { result, currentPly } = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
  )

  const [sessionState, setSessionState] = useState<ReviewSessionState>({
    status: 'running',
    error: null,
  })
  const [progress, setProgress] = useState<ReviewProgress>({
    stage: 'preparing',
    completed: 0,
    total: config.meta.plies + 1,
    currentPly: 0,
    phase: null,
    winPcts: [],
    cachedPositions: 0,
    enginePositions: 0,
  })
  const [orientation, setOrientation] = useState<'white' | 'black'>('white')

  const sessionRef = useRef<ReviewSession | null>(null)
  // A sessão atualiza seu buffer por posição; aqui o copiamos apenas uma vez por
  // frame, preservando o snapshot imutável que entra no estado do React.
  const pendingProgressRef = useRef<ReviewProgress>(progress)
  const rafRef = useRef<number | null>(null)

  useEffect(() => {
    const session = createReviewSession({
      config,
      backend: createTauriBackend(),
      store,
      onStateChange: (s) => {
        setSessionState(s)
      },
      onProgress: (nextProgress) => {
        pendingProgressRef.current = nextProgress
        if (rafRef.current == null) {
          rafRef.current = requestAnimationFrame(() => {
            rafRef.current = null
            const pending = pendingProgressRef.current
            setProgress({ ...pending, winPcts: pending.winPcts.slice() })
          })
        }
      },
    })
    sessionRef.current = session
    void session.start()
    return () => {
      if (rafRef.current != null) {
        cancelAnimationFrame(rafRef.current)
        rafRef.current = null
      }
      sessionRef.current = null
      session.dispose()
    }
  }, [config, store])

  const goTo = useCallback((ply: number) => store.goTo(ply), [store])
  const next = useCallback(() => store.next(), [store])
  const prev = useCallback(() => store.prev(), [store])
  const first = useCallback(() => store.first(), [store])
  const last = useCallback(() => store.last(), [store])
  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )

  return {
    result,
    status: sessionState.status,
    error: sessionState.error,
    partialWinPcts: progress.winPcts,
    progress,
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
