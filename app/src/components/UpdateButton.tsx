import { Download, LoaderCircle } from 'lucide-react'
import { useStore } from 'zustand'
import { runUi } from '../lib/effect/ui-runtime'
import { updatesStore } from '../lib/updates-store'
import { Button } from './ui/button'

export default function UpdateButton() {
  const state = useStore(updatesStore)
  if (state.status !== 'available') return null

  const progress = state.progress
  const restartRequired = progress?.status === 'restartRequired'
  let label = `Atualizar para ${state.info?.availableVersion} e reiniciar`
  let percent = 0
  if (progress?.status === 'downloading') {
    label = 'Baixando atualização…'
    if (progress.total) {
      percent = Math.min(
        100,
        Math.round((progress.downloaded / progress.total) * 100),
      )
      label = `Baixando atualização… ${percent}%`
    }
  }
  if (progress?.status === 'installing') label = 'Instalando atualização…'
  if (progress?.status === 'restarting') label = 'Reiniciando o EngineRoom…'
  if (restartRequired) label = 'Reiniciar o app'

  return (
    <div className='relative inline-flex'>
      <Button
        onClick={() => {
          void runUi(restartRequired ? state.restart() : state.install())
        }}
        variant='ghost'
        size='icon'
        className='relative size-10 rounded-xl text-brand disabled:opacity-100'
        disabled={state.busy}
        aria-label={label}
        aria-busy={state.busy}
        title={label}
      >
        {progress?.status === 'downloading' ? (
          <span
            className='relative grid size-8 place-items-center'
            aria-hidden='true'
          >
            <svg
              className='absolute inset-0 size-full -rotate-90'
              viewBox='0 0 32 32'
              fill='none'
              aria-hidden='true'
            >
              <circle
                cx='16'
                cy='16'
                r='14'
                stroke='currentColor'
                strokeWidth='1.5'
                className='opacity-15'
              />
              <circle
                cx='16'
                cy='16'
                r='14'
                stroke='currentColor'
                strokeWidth='1.5'
                strokeLinecap='round'
                pathLength='100'
                strokeDasharray='100'
                strokeDashoffset={100 - percent}
                className='transition-[stroke-dashoffset] duration-200 motion-reduce:transition-none'
              />
            </svg>
            <Download
              size={16}
              strokeWidth={1.8}
              className='update-download-motion'
              aria-hidden='true'
            />
          </span>
        ) : state.busy ? (
          <LoaderCircle size={18} className='animate-spin' aria-hidden='true' />
        ) : (
          <Download size={18} strokeWidth={1.8} aria-hidden='true' />
        )}
      </Button>
      <span className='sr-only' role='status'>
        {state.busy ? label : ''}
      </span>
      {state.error && (
        <p
          role='alert'
          className='absolute right-0 top-full z-50 mt-2 w-64 rounded-xl border border-edge bg-panel-2 p-3 text-xs text-blunder shadow-lg'
        >
          {state.error}
        </p>
      )}
    </div>
  )
}
