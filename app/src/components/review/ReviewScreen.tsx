import {
  ArrowUpDown,
  ChartLine,
  ChevronLeft,
  ChevronRight,
  SkipBack,
  SkipForward,
  TriangleAlert,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useStore } from 'zustand'
import { evalLabel } from '../../lib/eval-label'
import { phaseBoundaries } from '../../lib/phase'
import {
  selectDisplayedFen,
  selectDisplayedPosition,
  selectVariationMove,
} from '../../lib/review-selectors'
import { useSettings } from '../../lib/settings-context'
import { playMoveSound } from '../../lib/sound'
import { useReview } from '../../lib/use-review'
import type { Classification, ReviewConfig } from '../../types'
import Board from '../Board'
import CandidateLines from '../CandidateLines'
import EvalBar from '../EvalBar'
import EvalGraph from '../EvalGraph'
import ReviewAnalysisModal from './ReviewAnalysisModal'
import ReviewHeader from './ReviewHeader'
import ReviewLiveStatus from './ReviewLiveStatus'
import ReviewLoading from './ReviewLoading'
import { PlayerTag, ReviewNavButton } from './ReviewNavigation'
import ReviewSidebar from './ReviewSidebar'

interface ReviewScreenProps {
  config: ReviewConfig
  onExit: () => void
}

const ANALYSIS_ARROW_BRUSHES = [
  'blue',
  'analysisBlueMedium',
  'analysisBlueFaint',
] as const
const EXPLORE_STEP_MS = 650

function uciToSquares(uci: string): [string, string] | null {
  if (uci.length < 4) return null
  return [uci.slice(0, 2), uci.slice(2, 4)]
}

export default function ReviewScreen({ config, onExit }: ReviewScreenProps) {
  const review = useReview(config)
  const settings = useSettings(({ settings }) => ({
    reviewEngineEnabled: settings.reviewEngineEnabled,
    reviewMoveFeedbackEnabled: settings.reviewMoveFeedbackEnabled,
    reviewAnalysisLines: settings.reviewAnalysisLines,
    soundEnabled: settings.soundEnabled,
    soundVolume: settings.soundVolume,
  }))
  const { result, status, error, partialWinPcts, currentPly, orientation } =
    review
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const [analysisModalOpen, setAnalysisModalOpen] = useState(false)
  const [isExploringLine, setIsExploringLine] = useState(false)
  const exploreTimerRef = useRef<number | null>(null)

  const stopExplorePlayback = useCallback(
    (endPlaybackAnalysis = true) => {
      if (exploreTimerRef.current !== null) {
        window.clearTimeout(exploreTimerRef.current)
        exploreTimerRef.current = null
      }
      setIsExploringLine(false)
      if (endPlaybackAnalysis) review.endPlaybackAnalysis()
    },
    [review.endPlaybackAnalysis],
  )

  const exploreLineSlowly = useCallback(
    (pv: string[]) => {
      stopExplorePlayback(false)
      review.startPlaybackAnalysis()
      setIsExploringLine(true)
      const path = review.exploreLine(pv)
      const variation = review.store.getState().variation
      if (!variation || path.length === 0) {
        stopExplorePlayback()
        return
      }
      let cursor = variation.path.length
      let remainingMoves = path.length - cursor
      const advance = () => {
        review.goToVariation(variation.id, path.slice(0, ++cursor))
        remainingMoves--
        if (remainingMoves === 0) {
          exploreTimerRef.current = null
          setIsExploringLine(false)
          review.endPlaybackAnalysis()
          return
        }
        exploreTimerRef.current = window.setTimeout(advance, EXPLORE_STEP_MS)
      }
      if (remainingMoves > 0) {
        exploreTimerRef.current = window.setTimeout(advance, EXPLORE_STEP_MS)
      } else {
        setIsExploringLine(false)
        review.endPlaybackAnalysis()
      }
    },
    [
      review.exploreLine,
      review.store,
      review.goToVariation,
      review.startPlaybackAnalysis,
      review.endPlaybackAnalysis,
      stopExplorePlayback,
    ],
  )

  useEffect(
    () => () => {
      if (exploreTimerRef.current !== null) {
        window.clearTimeout(exploreTimerRef.current)
      }
    },
    [],
  )

  useEffect(() => {
    if (status !== 'running') return
    const startedAt = Date.now()
    const timer = window.setInterval(
      () => setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000)),
      1000,
    )
    return () => window.clearInterval(timer)
  }, [status])

  const variationMove = useStore(review.store, selectVariationMove)
  const displayedFen = useStore(review.store, selectDisplayedFen)
  const displayedPosition = useStore(review.store, selectDisplayedPosition)
  const stm = displayedFen?.split(' ')[1] === 'b' ? 'b' : 'w'
  const currentMove =
    currentPly > 0 ? (result?.moves[currentPly - 1] ?? null) : null
  const lastMoveUci = currentMove?.uci ?? null
  let lastMoveClassification: Classification | null =
    currentMove?.classification ?? null
  if (review.variation) {
    lastMoveClassification = settings.reviewMoveFeedbackEnabled
      ? (variationMove?.classification ?? null)
      : null
  }

  const opening = useMemo(
    function findOpening() {
      return result?.moves.find((move) => move.eco)?.eco ?? null
    },
    [result],
  )
  const evalBarLabel =
    displayedPosition && result
      ? evalLabel(displayedPosition.cp, displayedPosition.fen, stm)
      : undefined

  const [selectedMultipv, setSelectedMultipv] = useState(1)
  // biome-ignore lint/correctness/useExhaustiveDependencies: reseta a linha selecionada sempre que o usuário navega para outro lance
  useEffect(() => {
    setSelectedMultipv(1)
  }, [displayedFen, settings.reviewAnalysisLines])

  // Toca o som do lance apenas ao avançar (currentPly aumenta). Voltar/início
  // não dispara som. Ref rastreia o ply anterior sem causar re-render.
  const prevPlyRef = useRef(currentPly)
  // biome-ignore lint/correctness/useExhaustiveDependencies: dispara só em mudança de ply; settings/result lidos via closure no último render
  useEffect(() => {
    const movedForward = currentPly > prevPlyRef.current
    prevPlyRef.current = currentPly
    if (!movedForward || !settings.soundEnabled) return
    const san = result?.moves[currentPly - 1]?.san
    if (san) playMoveSound(san, settings.soundVolume)
  }, [currentPly])
  const lastMove = useMemo(
    function lastMoveSquares() {
      if (!lastMoveUci) return null
      return uciToSquares(lastMoveUci)
    },
    [lastMoveUci],
  )
  const arrows = useMemo(
    function boardArrows() {
      return (displayedPosition?.lines ?? [])
        .slice(0, ANALYSIS_ARROW_BRUSHES.length)
        .flatMap((line, index) => {
          const squares = line.pv[0] ? uciToSquares(line.pv[0]) : null
          return squares
            ? [
                {
                  from: squares[0],
                  to: squares[1],
                  brush: ANALYSIS_ARROW_BRUSHES[index],
                },
              ]
            : []
        })
    },
    [displayedPosition],
  )
  const graph = useMemo(
    function graphData() {
      if (!result) return null
      const positions = result.positions
      return {
        winPcts: positions.map((position) => position.winPct),
        phases: phaseBoundaries(positions.map((position) => position.phase)),
      }
    },
    [result],
  )

  useEffect(() => {
    if (!result) return
    const onKey = (e: KeyboardEvent) => {
      stopExplorePlayback()
      switch (e.key) {
        case 'ArrowLeft':
          review.prev()
          break
        case 'ArrowRight':
          review.next()
          break
        case 'Home':
          review.first()
          break
        case 'End':
          review.last()
          break
        default:
          return
      }
      e.preventDefault()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [
    result,
    review.prev,
    review.next,
    review.first,
    review.last,
    stopExplorePlayback,
  ])

  if (status === 'running') {
    return (
      <ReviewLoading
        config={config}
        elapsedSeconds={elapsedSeconds}
        partialWinPcts={partialWinPcts}
        progress={review.progress}
      />
    )
  }

  return (
    <div className='mx-auto flex min-h-full max-w-6xl flex-col gap-4 px-4 py-6'>
      <ReviewHeader
        config={config}
        opening={opening}
        onExit={onExit}
        onOpenAnalysisSettings={() => setAnalysisModalOpen(true)}
      />

      <ReviewAnalysisModal
        open={analysisModalOpen}
        onClose={() => setAnalysisModalOpen(false)}
      />

      {status === 'error' && (
        <div className='flex items-center gap-2.5 rounded-xl border border-blunder/50 bg-blunder/10 p-4 text-sm text-blunder'>
          <TriangleAlert
            size={16}
            strokeWidth={2}
            shrink-0
            aria-hidden='true'
          />
          <span>Falha na análise: {error}</span>
        </div>
      )}

      <div className='grid flex-1 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px]'>
        <div className='flex flex-col gap-3'>
          <div className='flex flex-col gap-1'>
            <PlayerTag
              name={
                orientation === 'white' ? config.meta.black : config.meta.white
              }
              elo={
                orientation === 'white'
                  ? config.meta.blackElo
                  : config.meta.whiteElo
              }
              color={orientation === 'white' ? 'b' : 'w'}
            />
            <div className='flex items-stretch gap-2'>
              <EvalBar
                winPct={displayedPosition?.winPct ?? 50}
                orientation={orientation}
                label={evalBarLabel}
              />
              <div className='min-w-0 flex-1'>
                {displayedFen ? (
                  <Board
                    fen={displayedFen}
                    orientation={orientation}
                    lastMove={
                      variationMove ? uciToSquares(variationMove.uci) : lastMove
                    }
                    arrows={arrows}
                    onMove={(from, to, promotion) => {
                      stopExplorePlayback()
                      return review.makeMove(from, to, promotion)
                    }}
                    lastMoveClassification={lastMoveClassification}
                  />
                ) : (
                  <div className='flex aspect-square w-full items-center justify-center rounded-lg border border-edge bg-panel-2/60 text-ink-dim'>
                    —
                  </div>
                )}
              </div>
            </div>
            <PlayerTag
              name={
                orientation === 'white' ? config.meta.white : config.meta.black
              }
              elo={
                orientation === 'white'
                  ? config.meta.whiteElo
                  : config.meta.blackElo
              }
              color={orientation === 'white' ? 'w' : 'b'}
            />
          </div>

          {settings.reviewEngineEnabled && displayedPosition?.lines?.length ? (
            <CandidateLines
              lines={displayedPosition.lines.slice(
                0,
                settings.reviewAnalysisLines,
              )}
              selectedMultipv={selectedMultipv}
              onSelect={(multipv) => {
                stopExplorePlayback()
                setSelectedMultipv(multipv)
              }}
              fen={displayedPosition.fen}
              onExplore={exploreLineSlowly}
            />
          ) : null}

          {isExploringLine ? (
            <p className='px-1 text-xs text-ink-faint' role='status'>
              Reproduzindo linha · avaliando lances…
            </p>
          ) : null}

          {!isExploringLine && <ReviewLiveStatus store={review.store} />}

          <div className='surface-glass elev-card flex items-center justify-center gap-2 rounded-xl border border-edge p-2'>
            <ReviewNavButton
              onClick={() => {
                stopExplorePlayback()
                review.first()
              }}
              disabled={!result}
              label='Primeiro lance'
            >
              <SkipBack size={18} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <ReviewNavButton
              onClick={() => {
                stopExplorePlayback()
                review.prev()
              }}
              disabled={!result || (!review.variation && currentPly === 0)}
              label='Lance anterior'
            >
              <ChevronLeft size={20} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <ReviewNavButton
              onClick={() => {
                stopExplorePlayback()
                review.next()
              }}
              disabled={
                !result ||
                (review.variation
                  ? review.variation.path.length === 0
                    ? review.variation.roots.length === 0
                    : (variationMove?.children.length ?? 0) === 0
                  : currentPly >= (result?.moves.length ?? 0))
              }
              label='Próximo lance'
            >
              <ChevronRight size={20} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <ReviewNavButton
              onClick={() => {
                stopExplorePlayback()
                review.last()
              }}
              disabled={
                !result ||
                (!review.variation && currentPly >= (result?.moves.length ?? 0))
              }
              label='Último lance'
            >
              <SkipForward size={18} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <div className='mx-1 h-5 w-px bg-edge' />
            <ReviewNavButton onClick={review.flip} label='Virar o tabuleiro'>
              <ArrowUpDown size={17} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
          </div>
        </div>

        <ReviewSidebar store={review.store} onNavigate={stopExplorePlayback} />
      </div>

      {result && graph && (
        <div className='rounded-xl border border-edge bg-panel-2/60 p-3 sm:p-4'>
          <div className='mb-2 flex flex-col gap-1 px-1 sm:flex-row sm:items-center sm:justify-between'>
            <span className='flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-ink-faint'>
              <ChartLine size={12} strokeWidth={2.2} aria-hidden='true' />
              Avaliação
            </span>
            <span className='hidden text-[11px] text-ink-faint sm:block'>
              clique para pular até o lance
            </span>
          </div>
          <EvalGraph
            winPcts={graph.winPcts}
            currentPly={currentPly}
            onSelect={(ply) => {
              stopExplorePlayback()
              review.goTo(ply)
            }}
            phases={graph.phases}
          />
        </div>
      )}
    </div>
  )
}
