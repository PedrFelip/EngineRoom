/**
 * Ponte React da revisão: o estado e as transições moram no `ReviewStore`
 * (puro) e a orquestração de I/O na `ReviewSession` (seam `Backend`); este
 * hook é só cola de view — bridging de snapshot, coalescing de progresso em
 * rAF, orientação do tabuleiro e derivações baratas para o chessground.
 */

import { Chess } from 'chess.js'
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react'
import type {
  ReviewConfig,
  ReviewResult,
  Variation,
  VariationMap,
  VariationMove,
} from '../types'
import { createTauriBackend } from './backend'
import {
  createReviewSession,
  type ReviewSession,
  type ReviewSessionState,
} from './review-session'
import { createReviewStore, type ReviewStore } from './review-store'
import { useSettings } from './settings-context'

export interface UseReview {
  result: ReviewResult | null
  status: ReviewSessionState['status']
  error: string | null
  /** winPcts parciais (POV brancas) que alimentam o gráfico durante o loading. */
  partialWinPcts: number[]
  currentPly: number
  orientation: 'white' | 'black'
  goTo: (ply: number) => void
  next: () => void
  prev: () => void
  first: () => void
  last: () => void
  flip: () => void
  /** Linhas alternativas jogadas pelo usuário, indexadas pelo ply-pai. */
  variations: VariationMap
  /** Variação/ply atualmente em foco (null = linha principal). */
  currentVariation: {
    variationId: string
    parentPly: number
    ply: number
  } | null
  /** Destinos lícitos do lado a jogar na posição exibida (para o chessground). */
  dests: Map<string, string[]> | null
  /** Cor do lado a jogar na posição exibida. */
  turnColor: 'white' | 'black' | null
  /** Lance (de variação ou linha principal) que levou à posição exibida. */
  currentVariationMove: VariationMove | null
  /** Aplica um lance arrastado pelo usuário: avança a linha ou abre variação. */
  makeMove: (uci: string) => void
  /** Navega para um lance de uma variação. */
  goToVariation: (variationId: string, parentPly: number, ply: number) => void
  /** Abandona a variação e volta para a linha principal no ply-pai. */
  exitVariation: () => void
}

export function useReview(config: ReviewConfig): UseReview {
  const { settings } = useSettings()

  // O store sobrevive a re-runs do effect de sessão (ex.: troca de enginePath):
  // mesma semântica dos useStates espalhados que ele substitui — trocar a
  // engine não perde o cursor nem as variações exploradas.
  const storeRef = useRef<ReviewStore | null>(null)
  if (!storeRef.current) storeRef.current = createReviewStore()
  const store = storeRef.current
  const { result, currentPly, variations, currentVariation } =
    useSyncExternalStore(store.subscribe, store.getSnapshot)

  const [sessionState, setSessionState] = useState<ReviewSessionState>({
    status: 'running',
    error: null,
  })
  const [partialWinPcts, setPartialWinPcts] = useState<number[]>([])
  const [orientation, setOrientation] = useState<'white' | 'black'>('white')

  const sessionRef = useRef<ReviewSession | null>(null)
  // Buffer de winPcts parciais + id do rAF pendente: coalesce várias posições
  // (ex.: cache hits rápidos) num único setState por frame — loading suave.
  const pendingWinPctsRef = useRef<number[]>([])
  const rafRef = useRef<number | null>(null)

  useEffect(() => {
    const session = createReviewSession({
      config,
      enginePath: settings.enginePath,
      backend: createTauriBackend(),
      store,
      onStateChange: (s) => {
        if (s.status === 'done') setPartialWinPcts([])
        setSessionState(s)
      },
      onProgress: (wp) => {
        pendingWinPctsRef.current = wp
        if (rafRef.current == null) {
          rafRef.current = requestAnimationFrame(() => {
            rafRef.current = null
            setPartialWinPcts(pendingWinPctsRef.current)
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
  }, [config, settings.enginePath, store])

  // Variação em foco e lance atual dentro dela (null = linha principal).
  const currentVariationData = useMemo<Variation | null>(() => {
    if (!currentVariation) return null
    const list = variations[currentVariation.parentPly] ?? []
    return list.find((v) => v.id === currentVariation.variationId) ?? null
  }, [currentVariation, variations])

  const currentVariationMove = useMemo<VariationMove | null>(() => {
    if (!currentVariation || !currentVariationData) return null
    return currentVariationData.moves[currentVariation.ply - 1] ?? null
  }, [currentVariation, currentVariationData])

  const displayedFen =
    currentVariationMove?.fenAfter ?? result?.positions[currentPly]?.fen ?? null

  // Reponta o refino ao vivo. Depende só do FEN exibido — assim o refino
  // progressivo de uma mesma posição não é reiniciado quando o lance recebe nota.
  useEffect(() => {
    if (displayedFen) sessionRef.current?.setDisplayedFen(displayedFen)
  }, [displayedFen])

  // Derivação pura (era effect + setState): dests/turnColor da posição exibida.
  const { dests, turnColor } = useMemo<{
    dests: Map<string, string[]> | null
    turnColor: 'white' | 'black' | null
  }>(() => {
    if (!displayedFen) return { dests: null, turnColor: null }
    try {
      const chess = new Chess(displayedFen)
      const map = new Map<string, string[]>()
      for (const m of chess.moves({ verbose: true })) {
        const arr = map.get(m.from)
        if (arr) arr.push(m.to)
        else map.set(m.from, [m.to])
      }
      return {
        dests: map.size > 0 ? map : null,
        turnColor: chess.turn() === 'w' ? 'white' : 'black',
      }
    } catch {
      return { dests: null, turnColor: null }
    }
  }, [displayedFen])

  const goTo = useCallback((ply: number) => store.goTo(ply), [store])
  const next = useCallback(() => store.next(), [store])
  const prev = useCallback(() => store.prev(), [store])
  const first = useCallback(() => store.first(), [store])
  const last = useCallback(() => store.last(), [store])
  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )
  const goToVariation = useCallback(
    (variationId: string, parentPly: number, ply: number) =>
      store.goToVariation(variationId, parentPly, ply),
    [store],
  )
  const exitVariation = useCallback(() => store.exitVariation(), [store])
  const makeMove = useCallback((uci: string) => store.makeMove(uci), [store])

  return {
    result,
    status: sessionState.status,
    error: sessionState.error,
    partialWinPcts,
    currentPly,
    orientation,
    goTo,
    next,
    prev,
    first,
    last,
    flip,
    variations,
    currentVariation,
    dests,
    turnColor,
    currentVariationMove,
    makeMove,
    goToVariation,
    exitVariation,
  }
}
