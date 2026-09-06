import { ArrowLeft, MoreHorizontal } from 'lucide-react'
import { formatEngineTag } from '../../lib/engine-tag'
import { resultLabel } from '../../lib/pgn'
import { useSettings } from '../../lib/settings-context'
import type { ReviewConfig } from '../../types'
import { Button } from '../ui/button'
import { Switch } from '../ui/switch'

interface Props {
  config: ReviewConfig
  opening: { code: string; name: string } | null
  onExit: () => void
  onOpenAnalysisSettings: () => void
}

export default function ReviewHeader({
  config,
  opening,
  onExit,
  onOpenAnalysisSettings,
}: Props) {
  const reviewEngineEnabled = useSettings(
    (state) => state.settings.reviewEngineEnabled,
  )
  const updateSettings = useSettings((state) => state.updateSettings)

  return (
    <header className='flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between sm:gap-4'>
      <div className='min-w-0'>
        <p className='section-kicker mb-1'>análise de partida</p>
        <h1 className='truncate text-lg font-bold text-ink'>
          {config.meta.white} <span className='text-ink-faint'>vs</span>{' '}
          {config.meta.black}
        </h1>
        <p className='truncate text-sm text-ink-dim'>
          {resultLabel(config.meta.result)} · {Math.ceil(config.meta.plies / 2)}{' '}
          lances ·{' '}
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
      <div className='flex w-full shrink-0 items-center gap-2 sm:w-auto'>
        <div className='flex h-9 items-center overflow-hidden rounded-xl border border-edge bg-panel-2/80 shadow-sm backdrop-blur'>
          <label
            htmlFor='review-analysis-switch'
            className='flex h-full cursor-pointer items-center gap-2 px-3 text-sm font-medium text-ink transition hover:bg-panel-3/70'
          >
            <Switch
              id='review-analysis-switch'
              size='sm'
              checked={reviewEngineEnabled}
              onCheckedChange={(reviewEngineEnabled) =>
                updateSettings({ reviewEngineEnabled })
              }
              aria-label='Ativar análise durante a revisão'
            />
            <span>Análise</span>
          </label>
          <span className='h-5 w-px bg-edge-soft' />
          <button
            type='button'
            onClick={onOpenAnalysisSettings}
            className='flex h-full w-10 items-center justify-center text-ink-dim transition hover:bg-panel-3 hover:text-ink'
            aria-label='Configurar análise'
          >
            <MoreHorizontal size={18} aria-hidden='true' />
          </button>
        </div>
        <Button
          onClick={onExit}
          variant='outline'
          className='ml-auto bg-panel-2 sm:ml-0'
        >
          <ArrowLeft size={16} strokeWidth={2} aria-hidden='true' />
          <span className='hidden sm:inline'>Nova partida</span>
        </Button>
      </div>
    </header>
  )
}
