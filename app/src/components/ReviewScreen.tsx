import {
  ArrowLeft,
  ArrowUpDown,
  Bot,
  ChartLine,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Crown,
  ListOrdered,
  SkipBack,
  SkipForward,
  TriangleAlert,
} from 'lucide-react'
import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import { formatEngineTag } from '../lib/engine-tag'
import { evalLabel, sideToMoveAtPly } from '../lib/eval-label'
import { resultLabel } from '../lib/pgn'
import { phaseBoundaries } from '../lib/phase'
import { useSettings } from '../lib/settings-context'
import { playMoveSound } from '../lib/sound'
import { useReview } from '../lib/use-review'
import type { Classification, ReviewConfig } from '../types'
import Board from './Board'
import CandidateLines from './CandidateLines'
import EvalBar from './EvalBar'
import EvalGraph from './EvalGraph'
import MoveList from './MoveList'
import ReviewSummary from './ReviewSummary'
import { Button } from './ui/button'

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
  const stm = result ? sideToMoveAtPly(result.moves, currentPly) : 'w'
  const currentMove =
    currentPly > 0 ? (result?.moves[currentPly - 1] ?? null) : null
  const lastMoveUci = currentMove?.uci ?? null
  const lastMoveClassification: Classification | null =
    currentMove?.classification ?? null

  const opening = result?.moves.find((m) => m.eco)?.eco ?? null
  const evalBarLabel =
    position && result ? evalLabel(position.cp, position.fen, stm) : undefined

  const [selectedMultipv, setSelectedMultipv] = useState(1)
  // biome-ignore lint/correctness/useExhaustiveDependencies: reseta a linha selecionada sempre que o usuário navega para outro lance
  useEffect(() => {
    setSelectedMultipv(1)
  }, [currentPly])

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
    const uci = selectedLine?.pv[0]
    if (!uci) return null
    const sq = uciToSquares(uci)
    return sq ? { from: sq[0], to: sq[1], brush: 'blue' as const } : null
  }, [selectedLine])

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
    const { progress } = review
    const progressPct =
      progress.total > 0
        ? Math.round((progress.completed / progress.total) * 100)
        : 0
    const phaseLabel = progress.phase
      ? PHASE_LABELS[progress.phase]
      : 'Preparando partida'
    const stageLabel = STAGE_LABELS[progress.stage]
    return (
      <div className='flex min-h-full items-center justify-center px-4 py-10'>
        <div className='w-full max-w-3xl'>
          <div className='mb-2 flex items-center justify-center gap-2 text-xs font-semibold tracking-[0.14em] text-brand uppercase'>
            <span className='engine-loading-orb h-2 w-2 animate-pulse rounded-full bg-brand' />
            Engine em análise
          </div>
          <h1 className='mb-1 text-center text-xl font-bold text-ink'>
            {config.meta.white} <span className='text-ink-faint'>vs</span>{' '}
            {config.meta.black}
          </h1>
          <p className='mb-5 text-center text-sm text-ink-dim'>
            {Math.ceil(config.meta.plies / 2)} lances ·{' '}
            {formatEngineTag({
              mode: config.mode,
              depth:
                config.mode === 'time'
                  ? (config.movetimeMs ?? 0)
                  : config.engine.depth,
              engineTier: config.engine.id,
              analysisKind: config.analysisKind,
            })}
          </p>

          <div className='elev-card mb-4 rounded-2xl border border-edge bg-panel-2/70 p-5'>
            <div className='mb-3 flex items-end justify-between gap-4'>
              <div>
                <p className='text-sm font-semibold text-ink'>{stageLabel}</p>
                <p className='mt-0.5 text-xs text-ink-dim'>
                  {progress.stage === 'preparing'
                    ? 'Configurando o motor e consultando o cache'
                    : progress.stage === 'finalizing'
                      ? 'Calculando precisão e classificações'
                      : `${phaseLabel} · posição ${progress.completed} de ${progress.total}`}
                </p>
              </div>
              <span className='font-mono text-xl font-bold tabular-nums text-brand'>
                {progressPct}%
              </span>
            </div>

            <div
              className='h-2 overflow-hidden rounded-full bg-panel-3'
              role='progressbar'
              aria-label={stageLabel}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={progressPct}
            >
              <div
                className='h-full rounded-full bg-brand transition-[width] duration-300 ease-out'
                style={{ width: `${progressPct}%` }}
              />
            </div>

            <div className='mt-4 grid grid-cols-1 gap-2 sm:grid-cols-3'>
              <LoadingStat label='Fase do jogo' value={phaseLabel} />
              <LoadingStat
                label='Progresso'
                value={`${progress.completed}/${progress.total}`}
              />
              <LoadingStat
                icon={<Clock3 size={13} aria-hidden='true' />}
                label='Tempo decorrido'
                value={formatDuration(elapsedSeconds)}
              />
            </div>
          </div>

          {partialWinPcts.length >= 2 ? (
            <div className='eval-graph-loading elev-card rounded-2xl border border-edge bg-panel-2/60 p-5'>
              <EvalGraph
                winPcts={partialWinPcts}
                currentPly={partialWinPcts.length - 1}
                onSelect={() => {}}
                pulse
              />
            </div>
          ) : (
            <p className='flex items-center justify-center gap-2 text-sm text-ink-dim'>
              <Bot
                size={16}
                strokeWidth={2}
                className='animate-pulse'
                aria-hidden='true'
              />
              {stageLabel}…
            </p>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className='mx-auto flex min-h-full max-w-6xl flex-col gap-4 px-4 py-6'>
      <header className='flex items-center justify-between gap-4'>
        <div className='min-w-0'>
          <p className='section-kicker mb-1'>análise de partida</p>
          <h1 className='truncate text-lg font-bold text-ink'>
            {config.meta.white} <span className='text-ink-faint'>vs</span>{' '}
            {config.meta.black}
          </h1>
          <p className='truncate text-sm text-ink-dim'>
            {resultLabel(config.meta.result)} ·{' '}
            {Math.ceil(config.meta.plies / 2)} lances ·{' '}
            {formatEngineTag({
              mode: config.mode,
              depth:
                config.mode === 'time'
                  ? (config.movetimeMs ?? 0)
                  : config.engine.depth,
              engineTier: config.engine.id,
              analysisKind: config.analysisKind,
            })}
            {opening ? ` · ${opening.code} ${opening.name}` : ''}
          </p>
        </div>
        <Button onClick={onExit} variant='outline' className='bg-panel-2'>
          <ArrowLeft size={16} strokeWidth={2} aria-hidden='true' />
          Nova partida
        </Button>
      </header>

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
                {position ? (
                  <Board
                    fen={position.fen}
                    orientation={orientation}
                    lastMove={lastMoveUci ? uciToSquares(lastMoveUci) : null}
                    arrows={bestArrow ? [bestArrow] : []}
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

          {position?.lines?.length ? (
            <CandidateLines
              lines={position.lines}
              selectedMultipv={selectedMultipv}
              onSelect={setSelectedMultipv}
            />
          ) : null}

          <div className='surface-glass elev-card flex items-center justify-center gap-2 rounded-xl border border-edge p-2'>
            <NavBtn
              onClick={review.first}
              disabled={!result}
              label='Primeiro lance'
            >
              <SkipBack size={18} strokeWidth={2} aria-hidden='true' />
            </NavBtn>
            <NavBtn
              onClick={review.prev}
              disabled={!result || currentPly === 0}
              label='Lance anterior'
            >
              <ChevronLeft size={20} strokeWidth={2} aria-hidden='true' />
            </NavBtn>
            <NavBtn
              onClick={review.next}
              disabled={!result || currentPly >= (result?.moves.length ?? 0)}
              label='Próximo lance'
            >
              <ChevronRight size={20} strokeWidth={2} aria-hidden='true' />
            </NavBtn>
            <NavBtn
              onClick={review.last}
              disabled={!result || currentPly >= (result?.moves.length ?? 0)}
              label='Último lance'
            >
              <SkipForward size={18} strokeWidth={2} aria-hidden='true' />
            </NavBtn>
            <div className='mx-1 h-5 w-px bg-edge' />
            <NavBtn onClick={review.flip} label='Virar o tabuleiro'>
              <ArrowUpDown size={17} strokeWidth={2} aria-hidden='true' />
            </NavBtn>
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
                  currentPly={currentPly}
                  onSelect={review.goTo}
                />
              </div>
            </div>
          )}
        </aside>
      </div>

      {result && (
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
            winPcts={result.positions.map((p) => p.winPct)}
            currentPly={currentPly}
            onSelect={review.goTo}
            phases={phaseBoundaries(result.positions.map((p) => p.phase))}
          />
        </div>
      )}
    </div>
  )
}

const PHASE_LABELS = {
  opening: 'Abertura',
  middlegame: 'Meio-jogo',
  endgame: 'Final',
} as const

const STAGE_LABELS = {
  preparing: 'Iniciando Stockfish',
  analyzing: 'Analisando a partida',
  triage: 'Triagem da partida',
  refinement: 'Refinando lances críticos',
  finalizing: 'Finalizando a revisão',
} as const

function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.round(totalSeconds))
  const minutes = Math.floor(seconds / 60)
  const rest = seconds % 60
  return minutes > 0 ? `${minutes}min ${rest}s` : `${rest}s`
}

function LoadingStat({
  icon,
  label,
  value,
}: {
  icon?: ReactNode
  label: string
  value: string
}) {
  return (
    <div className='rounded-lg border border-edge-soft bg-panel-3/45 px-3 py-2.5'>
      <span className='flex items-center gap-1 text-[10px] font-semibold tracking-wide text-ink-faint uppercase'>
        {icon}
        {label}
      </span>
      <span className='mt-1 block truncate text-sm font-semibold text-ink'>
        {value}
      </span>
    </div>
  )
}

function NavBtn({
  onClick,
  disabled,
  label,
  children,
}: {
  onClick: () => void
  disabled?: boolean
  label: string
  children: ReactNode
}) {
  return (
    <Button
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
      variant='ghost'
      size='icon'
      className='text-ink-dim disabled:opacity-30'
    >
      {children}
    </Button>
  )
}

function PlayerTag({
  name,
  elo,
  color,
}: {
  name: string
  elo: string | null
  color: 'w' | 'b'
}) {
  return (
    <div className='flex items-center gap-2 px-1 text-sm'>
      <span
        className='flex h-5 w-5 shrink-0 items-center justify-center rounded-full ring-1 ring-edge'
        style={{
          backgroundColor:
            color === 'w' ? 'var(--piece-white-bg)' : 'var(--piece-black-bg)',
          color:
            color === 'w' ? 'var(--piece-white-fg)' : 'var(--piece-black-fg)',
        }}
      >
        <Crown size={11} strokeWidth={2.5} aria-hidden='true' />
      </span>
      <span className='font-medium text-ink'>{name}</span>
      {elo ? (
        <span className='font-mono text-xs text-ink-dim'>({elo})</span>
      ) : null}
    </div>
  )
}
