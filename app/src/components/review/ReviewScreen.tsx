import {
  ArrowUpDown,
  ChartLine,
  ChevronLeft,
  ChevronRight,
  ListOrdered,
  SkipBack,
  SkipForward,
  TriangleAlert,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { evalLabel } from '../../lib/eval-label'
import { phaseBoundaries } from '../../lib/phase'
import { nodeAtPath } from '../../lib/review-store'
import { useSettings } from '../../lib/settings-context'
import { playMoveSound } from '../../lib/sound'
import { useReview } from '../../lib/use-review'
import type { Classification, ReviewConfig } from '../../types'
import Board from '../Board'
import CandidateLines from '../CandidateLines'
import EvalBar from '../EvalBar'
import EvalGraph from '../EvalGraph'
import MoveList from '../MoveList'
import ReviewSummary from '../ReviewSummary'
import ReviewAnalysisModal from './ReviewAnalysisModal'
import ReviewHeader from './ReviewHeader'
import ReviewLoading from './ReviewLoading'
import { PlayerTag, ReviewNavButton } from './ReviewNavigation'

interface ReviewScreenProps {
  config: ReviewConfig
  onExit: () => void
}

function uciToSquares(uci: string): [string, string] | null {
  if (uci.length < 4) return null
  return [uci.slice(0, 2), uci.slice(2, 4)]
}

export default function ReviewScreen({ config, onExit }: ReviewScreenProps) {
  const review = useReview(config)
  const { settings } = useSettings()
  const { result, status, error, partialWinPcts, currentPly, orientation } =
    review
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const [analysisModalOpen, setAnalysisModalOpen] = useState(false)

  useEffect(() => {
    if (status !== 'running') return
    const startedAt = Date.now()
    const timer = window.setInterval(
      () => setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000)),
      1000,
    )
    return () => window.clearInterval(timer)
  }, [status])

  const position = result?.positions[currentPly] ?? null
  const variationMove = review.variation
    ? nodeAtPath(review.variation.roots, review.variation.path)
    : null
  const displayedFen = variationMove?.fen ?? position?.fen ?? null
  const stm = displayedFen?.split(' ')[1] === 'b' ? 'b' : 'w'
  const currentMove =
    currentPly > 0 ? (result?.moves[currentPly - 1] ?? null) : null
  const lastMoveUci = currentMove?.uci ?? null
  const lastMoveClassification: Classification | null =
    currentMove?.classification ?? null

  const opening = useMemo(
    function findOpening() {
      return result?.moves.find((move) => move.eco)?.eco ?? null
    },
    [result],
  )
  const evalBarLabel =
    position && result ? evalLabel(position.cp, position.fen, stm) : undefined

  const [selectedMultipv, setSelectedMultipv] = useState(1)
  // biome-ignore lint/correctness/useExhaustiveDependencies: reseta a linha selecionada sempre que o usuário navega para outro lance
  useEffect(() => {
    setSelectedMultipv(1)
  }, [currentPly, settings.reviewAnalysisLines])

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
  const selectedLine =
    position?.lines.find((l) => l.multipv === selectedMultipv) ??
    position?.lines[0]

  const bestArrow = useMemo(() => {
    const uci = review.variation ? null : selectedLine?.pv[0]
    if (!uci) return null
    const sq = uciToSquares(uci)
    return sq ? { from: sq[0], to: sq[1], brush: 'blue' as const } : null
  }, [review.variation, selectedLine])
  const lastMove = useMemo(
    function lastMoveSquares() {
      if (!lastMoveUci) return null
      return uciToSquares(lastMoveUci)
    },
    [lastMoveUci],
  )
  const arrows = useMemo(
    function boardArrows() {
      return bestArrow ? [bestArrow] : []
    },
    [bestArrow],
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
  }, [result, review.prev, review.next, review.first, review.last])

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
                winPct={position?.winPct ?? 50}
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
                    onMove={review.makeMove}
                    lastMoveClassification={
                      review.variation ? null : lastMoveClassification
                    }
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

          {!review.variation &&
          settings.reviewEngineEnabled &&
          position?.lines?.length ? (
            <CandidateLines
              lines={position.lines.slice(0, settings.reviewAnalysisLines)}
              selectedMultipv={selectedMultipv}
              onSelect={setSelectedMultipv}
              fen={position.fen}
              onExplore={review.exploreLine}
            />
          ) : null}

          <div className='surface-glass elev-card flex items-center justify-center gap-2 rounded-xl border border-edge p-2'>
            <ReviewNavButton
              onClick={review.first}
              disabled={!result}
              label='Primeiro lance'
            >
              <SkipBack size={18} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <ReviewNavButton
              onClick={review.prev}
              disabled={!result || (!review.variation && currentPly === 0)}
              label='Lance anterior'
            >
              <ChevronLeft size={20} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <ReviewNavButton
              onClick={review.next}
              disabled={
                !result ||
                (review.variation
                  ? (variationMove?.children.length ?? 0) === 0
                  : currentPly >= (result?.moves.length ?? 0))
              }
              label='Próximo lance'
            >
              <ChevronRight size={20} strokeWidth={2} aria-hidden='true' />
            </ReviewNavButton>
            <ReviewNavButton
              onClick={review.last}
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

        <aside className='flex flex-col gap-4'>
          {result && <ReviewSummary result={result} />}
          {result && (
            <div className='rounded-xl border border-border bg-card/60 p-2 shadow-sm'>
              <div className='flex items-center justify-between px-1 py-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase'>
                <span className='flex items-center gap-1.5'>
                  <ListOrdered size={13} strokeWidth={2.2} aria-hidden='true' />
                  Lances
                </span>
                <span className='font-mono text-[10px] normal-case'>
                  {Math.ceil(result.moves.length / 2)}
                </span>
              </div>
              <div className='max-h-[42vh] overflow-y-auto pr-1 lg:max-h-[50vh]'>
                <MoveList
                  moves={result.moves}
                  currentPly={review.variation ? -1 : currentPly}
                  onSelect={review.goTo}
                  onBranchFrom={review.goTo}
                  variations={review.variations}
                  activeVariation={review.variation}
                  onSelectVariation={review.goToVariation}
                />
              </div>
            </div>
          )}
        </aside>
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
            onSelect={review.goTo}
            phases={graph.phases}
          />
        </div>
      )}
    </div>
  )
}
