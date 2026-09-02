import { Bot, Settings } from 'lucide-react'
import { Button } from '../ui/button'

interface Props {
  onOpenSettings: () => void
}

export default function HomeHeader({ onOpenSettings }: Props) {
  return (
    <header className='mb-8 flex w-full max-w-xl items-center justify-between md:max-w-6xl'>
      <div className='flex items-center gap-2.5'>
        <div className='brand-mark flex h-9 w-9 items-center justify-center rounded-lg bg-brand text-bg'>
          <Bot size={20} strokeWidth={2} aria-hidden='true' />
        </div>
        <div className='leading-tight'>
          <h1 className='text-lg font-bold tracking-tight text-ink'>
            EngineRoom
          </h1>
          <p className='text-[11px] font-medium tracking-wide text-ink-faint'>
            LOCAL CHESS INTELLIGENCE
          </p>
        </div>
      </div>
      <Button
        onClick={onOpenSettings}
        variant='ghost'
        size='icon'
        className='border-edge bg-panel-2/60'
        aria-label='Configurações'
        title='Configurações'
      >
        <Settings size={18} strokeWidth={1.8} aria-hidden='true' />
      </Button>
    </header>
  )
}
