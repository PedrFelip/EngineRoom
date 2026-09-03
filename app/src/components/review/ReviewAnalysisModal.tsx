import { Cpu, X } from 'lucide-react'
import { useEffect } from 'react'
import ReviewEngineSettings from '../settings/ReviewEngineSettings'

interface Props {
  open: boolean
  onClose: () => void
}

export default function ReviewAnalysisModal({ open, onClose }: Props) {
  useEffect(() => {
    if (!open) return
    const previousOverflow = document.body.style.overflow
    const previousPaddingRight = document.body.style.paddingRight
    const scrollbarWidth =
      window.innerWidth - document.documentElement.clientWidth
    document.body.style.overflow = 'hidden'
    if (scrollbarWidth > 0) {
      document.body.style.paddingRight = `${scrollbarWidth}px`
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', closeOnEscape)
    return () => {
      window.removeEventListener('keydown', closeOnEscape)
      document.body.style.overflow = previousOverflow
      document.body.style.paddingRight = previousPaddingRight
    }
  }, [open, onClose])

  if (!open) return null

  return (
    <div className='fixed inset-0 z-50 flex items-center justify-center p-4'>
      <button
        type='button'
        className='dialog-backdrop absolute inset-0 cursor-default'
        onClick={onClose}
        aria-label='Fechar configurações da análise'
      />
      <div
        role='dialog'
        aria-modal='true'
        aria-labelledby='review-analysis-title'
        className='surface-glass elev-dialog relative z-10 w-full max-w-md overflow-hidden rounded-2xl border border-edge'
      >
        <div className='flex items-start justify-between border-b border-edge-soft px-5 py-4'>
          <div className='flex gap-3'>
            <span className='flex size-9 shrink-0 items-center justify-center rounded-xl bg-brand/12 text-brand ring-1 ring-brand/20'>
              <Cpu size={18} strokeWidth={2} aria-hidden='true' />
            </span>
            <div>
              <h2 id='review-analysis-title' className='font-semibold text-ink'>
                Análise na revisão
              </h2>
              <p className='mt-0.5 text-xs text-ink-faint'>
                Ajuste como as linhas alternativas serão exploradas.
              </p>
            </div>
          </div>
          <button
            type='button'
            onClick={onClose}
            className='rounded-lg p-2 text-ink-dim transition hover:bg-panel-3 hover:text-ink'
            aria-label='Fechar'
          >
            <X size={17} aria-hidden='true' />
          </button>
        </div>
        <div className='p-5'>
          <ReviewEngineSettings />
        </div>
      </div>
    </div>
  )
}
