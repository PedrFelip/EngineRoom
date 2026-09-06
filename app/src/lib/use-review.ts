/**
 * Ponte React da revisão: o estado e as transições moram no `ReviewStore`
 * (puro) e a orquestração de I/O na `ReviewSession` (seam `Backend`); este
 * hook é só cola de view — bridging de snapshot, coalescing de progresso em
 * rAF e orientação do tabuleiro.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { useStore } from 'zustand'
import type { ReviewConfig, ReviewResult } from '../types'
import { createTauriBackend } from './backend'
import {
  selectDisplayedFen,
  selectSourceFen,
  selectSourcePosition,
  selectVariationMove,
} from './review-selectors'
import {
  createReviewSession,
  type ReviewProgress,
  type ReviewSession,
  type ReviewSessionState,
} from './review-session'
import {
  createReviewStore,
  type ReviewStore,
  type ReviewVariation,
} from './review-store'
import { useSettings } from './settings-context'
import { selectReviewEngineSettings } from './settings-store'

export interface UseReview {
  result: ReviewResult | null
  status: ReviewSessionState['status']
  error: string | null
  /** winPcts parciais (POV brancas) que alimentam o gráfico durante o loading. */
  partialWinPcts: readonly number[]
  progress: ReviewProgress
  currentPly: number
  orientation: 'white' | 'black'
  variation: ReviewVariation | null
  store: ReviewStore
  startPlaybackAnalysis: () => void
  endPlaybackAnalysis: () => void
  goTo: (ply: number) => void
  next: () => void
  prev: () => void
  first: () => void
  last: () => void
  flip: () => void
  makeMove: (from: string, to: string, promotion?: string) => boolean
  exploreLine: (pv: string[]) => void
  goToVariation: (variationId: string, path: string[]) => void
  exitVariation: () => void
}

export function useReview(config: ReviewConfig): UseReview {
  const settings = useSettings(selectReviewEngineSettings)
  const [store] = useState(createReviewStore)
  const result = useStore(store, (state) => state.result)
  const currentPly = useStore(store, (state) => state.currentPly)
  const variation = useStore(store, (state) => state.variation)

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
  const [playbackFastPass, setPlaybackFastPass] = useState(false)

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

  const variationNodeId = useStore(
    store,
    (state) => selectVariationMove(state)?.id,
  )
  const displayedFen = useStore(store, selectDisplayedFen)
  const sourcePosition = useStore(store, selectSourcePosition)
  const sourceFen = useStore(store, selectSourceFen)

  useEffect(() => {
    const session = sessionRef.current
    if (!session || !result || sessionState.status !== 'done') return
    if (!settings.reviewEngineEnabled || !displayedFen) {
      session.cancelLiveAnalysis()
      return
    }
    session.analyzePosition(
      {
        fen: displayedFen,
        variationNodeId,
        sourceFen,
        sourceAnalysis: sourcePosition,
      },
      {
        searchSeconds: settings.reviewSearchSeconds,
        lines: settings.reviewAnalysisLines,
        threadsAuto: settings.reviewThreadsAuto,
        threads: settings.reviewThreads,
        memoryMb: settings.reviewMemoryMb,
        moveFeedbackEnabled: settings.reviewMoveFeedbackEnabled,
        fastPass: playbackFastPass,
      },
    )
  }, [
    displayedFen,
    result,
    sessionState.status,
    settings.reviewEngineEnabled,
    settings.reviewThreadsAuto,
    settings.reviewMoveFeedbackEnabled,
    settings.reviewSearchSeconds,
    settings.reviewAnalysisLines,
    settings.reviewThreads,
    settings.reviewMemoryMb,
    playbackFastPass,
    sourceFen,
    sourcePosition,
    variationNodeId,
  ])

  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )
  const startPlaybackAnalysis = useCallback(() => {
    sessionRef.current?.cancelLiveAnalysis()
    setPlaybackFastPass(true)
  }, [])
  const endPlaybackAnalysis = useCallback(() => setPlaybackFastPass(false), [])

  return {
    result,
    status: sessionState.status,
    error: sessionState.error,
    partialWinPcts: progress.winPcts,
    progress,
    currentPly,
    orientation,
    variation,
    store,
    startPlaybackAnalysis,
    endPlaybackAnalysis,
    goTo: store.goTo,
    next: store.next,
    prev: store.prev,
    first: store.first,
    last: store.last,
    flip,
    makeMove: store.makeMove,
    exploreLine: store.exploreLine,
    goToVariation: store.goToVariation,
    exitVariation: store.exitVariation,
  }
}
