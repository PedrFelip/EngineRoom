import { Bot, Clock3 } from 'lucide-react'
import type { ReactNode } from 'react'
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
  triage: 'Triagem da partida',
  refinement: 'Refinando lances críticos',
  finalizing: 'Finalizando a revisão',
} as const

interface Props {
  config: ReviewConfig
  elapsedSeconds: number
  partialWinPcts: readonly number[]
  progress: ReviewProgress
}

export default function ReviewLoading({
  config,
  elapsedSeconds,
  partialWinPcts,
  progress,
}: Props) {
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
