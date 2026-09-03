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
import {
  createReviewStore,
  type LiveAnalysisState,
  nodeAtPath,
  type ReviewStore,
  type ReviewVariation,
} from './review-store'
import { useSettings } from './settings-context'

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
  variations: ReviewVariation[]
  liveAnalysis: LiveAnalysisState
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
  const { settings } = useSettings()
  const storeRef = useRef<ReviewStore | null>(null)
  if (!storeRef.current) storeRef.current = createReviewStore()
  const store = storeRef.current
  const { result, currentPly, variation, variations, liveAnalysis } =
    useSyncExternalStore(store.subscribe, store.getSnapshot)

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

  const variationMove = variation
    ? nodeAtPath(variation.roots, variation.path)
    : null
  const displayedFen =
    variationMove?.fen ?? result?.positions[currentPly]?.fen ?? null
  const parentMove =
    variation && variation.path.length > 1
      ? nodeAtPath(variation.roots, variation.path.slice(0, -1))
      : null
  const sourcePosition = variation
    ? (liveAnalysis.positions[
        parentMove?.fen ?? result?.positions[variation.basePly]?.fen ?? ''
      ] ?? result?.positions[variation.basePly])
    : undefined
  const sourceFen = variation
    ? (parentMove?.fen ?? result?.positions[variation.basePly]?.fen)
    : undefined

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
        variationNodeId: variationMove?.id,
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
    variationMove?.id,
  ])

  const goTo = useCallback((ply: number) => store.goTo(ply), [store])
  const next = useCallback(() => store.next(), [store])
  const prev = useCallback(() => store.prev(), [store])
  const first = useCallback(() => store.first(), [store])
  const last = useCallback(() => store.last(), [store])
  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )
  const makeMove = useCallback(
    (from: string, to: string, promotion?: string) =>
      store.makeMove(from, to, promotion),
    [store],
  )
  const exploreLine = useCallback(
    (pv: string[]) => store.exploreLine(pv),
    [store],
  )
  const goToVariation = useCallback(
    (variationId: string, path: string[]) =>
      store.goToVariation(variationId, path),
    [store],
  )
  const exitVariation = useCallback(() => store.exitVariation(), [store])
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
    variations,
    liveAnalysis,
    startPlaybackAnalysis,
    endPlaybackAnalysis,
    goTo,
    next,
    prev,
    first,
    last,
    flip,
    makeMove,
    exploreLine,
    goToVariation,
    exitVariation,
  }
}
