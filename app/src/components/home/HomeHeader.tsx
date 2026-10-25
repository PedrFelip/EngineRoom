import { Archive, Blocks, FilePlus2, Settings } from 'lucide-react'
import { Button } from '../ui/button'

interface Props {
  view: 'new' | 'history'
  onViewChange: (view: 'new' | 'history') => void
  onOpenSettings: () => void
}

export default function HomeHeader({
  view,
  onViewChange,
  onOpenSettings,
}: Props) {
  return (
    <header className='home-navigation flex w-full max-w-6xl flex-nowrap items-center justify-between gap-2 p-2.5 sm:gap-3 sm:p-3 md:px-4'>
      <h1 className='min-w-0 shrink'>
        <button
          type='button'
          onClick={() => onViewChange('new')}
          aria-label='EngineRoom — Nova análise'
          className='flex min-w-0 items-center gap-2.5 rounded-xl pl-1 text-left outline-none transition hover:opacity-80 focus-visible:ring-2 focus-visible:ring-brand'
        >
          <span className='flex size-9 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-brand'>
            <Blocks size={19} strokeWidth={1.8} aria-hidden='true' />
          </span>
          <span className='whitespace-nowrap text-sm font-semibold tracking-tight text-ink max-[359px]:sr-only'>
            EngineRoom
          </span>
        </button>
      </h1>
      <nav
        aria-label='Menu principal'
        className='flex shrink-0 items-center gap-0.5 sm:gap-1'
      >
        <button
          type='button'
          onClick={() => onViewChange('new')}
          aria-current={view === 'new' ? 'page' : undefined}
          className='home-nav-link'
        >
          <FilePlus2 size={16} strokeWidth={1.8} aria-hidden='true' />
          <span className='hidden sm:inline'>Nova análise</span>
          <span className='sr-only sm:hidden'>Nova análise</span>
        </button>
        <button
          type='button'
          onClick={() => onViewChange('history')}
          aria-current={view === 'history' ? 'page' : undefined}
          className='home-nav-link'
        >
          <Archive size={16} strokeWidth={1.8} aria-hidden='true' />
          <span className='hidden sm:inline'>Histórico</span>
          <span className='sr-only sm:hidden'>Histórico</span>
        </button>
        <span aria-hidden='true' className='mx-1 h-5 w-px bg-edge' />
        <Button
          onClick={onOpenSettings}
          variant='ghost'
          size='icon'
          className='size-10 rounded-xl'
          aria-label='Configurações'
          title='Configurações'
        >
          <Settings size={18} strokeWidth={1.8} aria-hidden='true' />
        </Button>
      </nav>
    </header>
  )
}
