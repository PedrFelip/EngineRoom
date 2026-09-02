import { ArrowLeft } from 'lucide-react'
import { formatEngineTag } from '../../lib/engine-tag'
import { resultLabel } from '../../lib/pgn'
import type { ReviewConfig } from '../../types'
import { Button } from '../ui/button'

interface Props {
  config: ReviewConfig
  opening: { code: string; name: string } | null
  onExit: () => void
}

export default function ReviewHeader({ config, opening, onExit }: Props) {
  return (
    <header className='flex items-center justify-between gap-4'>
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
      <Button onClick={onExit} variant='outline' className='bg-panel-2'>
        <ArrowLeft size={16} strokeWidth={2} aria-hidden='true' />
        Nova partida
      </Button>
    </header>
  )
}
