import { ArrowLeft, Clock3 } from 'lucide-react'
import { type ReactNode, useEffect, useMemo, useRef } from 'react'
import { ThinkingOrb } from 'thinking-orbs'
import { formatEngineTag } from '../../lib/engine-tag'
import type { ReviewProgress } from '../../lib/review-session'
import type { ReviewConfig } from '../../types'
import EvalGraph from '../EvalGraph'

const PHASE_LABELS = {
  opening: 'Abertura',
  middlegame: 'Meio-jogo',
  endgame: 'Final',
} as const

const STAGE_LABELS = {
  preparing: 'Iniciando Stockfish',
  analyzing: 'Analisando a partida',
  triage: 'Analisando e refinando a partida',
  refinement: 'Refinando lances críticos',
  finalizing: 'Finalizando a revisão',
} as const

interface Props {
  config: ReviewConfig
  elapsedSeconds: number
  partialWinPcts: readonly number[]
  progress: ReviewProgress
  onCancel: () => void
}

export default function ReviewLoading({
  config,
  elapsedSeconds,
  partialWinPcts,
  progress,
  onCancel,
}: Props) {
  const titleRef = useRef<HTMLHeadingElement>(null)
  useEffect(() => {
    titleRef.current?.focus({ preventScroll: true })
    titleRef.current?.scrollIntoView({ block: 'nearest' })
  }, [])

  const graphValues = useMemo(() => {
    const values = [...partialWinPcts]
    values.length = Math.max(values.length, config.meta.plies + 1)
    return values
  }, [partialWinPcts, config.meta.plies])
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
        <button
          type='button'
          onClick={onCancel}
          className='mb-8 flex items-center gap-2 rounded-xl px-3 py-2 text-sm text-ink-dim transition hover:bg-panel-3 hover:text-ink'
        >
          <ArrowLeft size={15} aria-hidden='true' />
          Cancelar e voltar
        </button>
        <div
          className='mb-4 hidden justify-center motion-safe:flex'
          aria-hidden='true'
        >
          <ThinkingOrb state='solving' size={64} />
        </div>
        <div className='mb-2 text-center text-xs font-semibold tracking-[0.14em] text-brand uppercase'>
          Engine em análise
        </div>
        <h1
          ref={titleRef}
          tabIndex={-1}
          className='mb-1 scroll-mt-24 rounded text-center text-xl font-bold text-ink focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-brand'
        >
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

        <div className='surface-glass elev-card mb-4 rounded-3xl border border-edge p-5 sm:p-7'>
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
              winPcts={graphValues}
              currentPly={progress.currentPly}
              onSelect={() => {}}
              pulse
            />
          </div>
        ) : (
          <p className='text-center text-sm text-ink-dim'>{stageLabel}…</p>
        )}
      </div>
    </div>
  )
}

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
