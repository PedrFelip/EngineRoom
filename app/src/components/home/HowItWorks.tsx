import { FileUp, ShieldCheck, SlidersHorizontal, Sparkles } from 'lucide-react'

export default function HowItWorks() {
  return (
    <aside id='how-it-works' className='w-full min-w-0 py-3 lg:px-3'>
      <p className='mb-2 text-xs font-medium text-ink-dim'>COMO FUNCIONA</p>
      <h2 className='mb-3 text-lg font-semibold tracking-tight text-ink'>
        Cada partida tem algo a ensinar.
      </h2>
      <div className='mt-4'>
        <ol className='grid gap-4 sm:grid-cols-3 lg:grid-cols-1 lg:gap-5'>
          <li className='flex gap-3'>
            <span className='flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-brand'>
              <FileUp size={14} strokeWidth={2.2} aria-hidden='true' />
            </span>
            <div>
              <p className='text-sm font-semibold text-ink'>
                Importe sua partida
              </p>
              <p className='mt-0.5 text-xs text-ink-dim'>
                Use um arquivo PGN, cole a notação ou busque no Chess.com.
              </p>
            </div>
          </li>
          <li className='flex gap-3'>
            <span className='flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-brand'>
              <SlidersHorizontal
                size={14}
                strokeWidth={2.2}
                aria-hidden='true'
              />
            </span>
            <div>
              <p className='text-sm font-semibold text-ink'>Escolha o ritmo</p>
              <p className='mt-0.5 text-xs text-ink-dim'>
                Use um perfil automático ou personalize sua análise.
              </p>
            </div>
          </li>
          <li className='flex gap-3'>
            <span className='flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-brand'>
              <Sparkles size={14} strokeWidth={2.2} aria-hidden='true' />
            </span>
            <div>
              <p className='text-sm font-semibold text-ink'>Receba a revisão</p>
              <p className='mt-0.5 text-xs text-ink-dim'>
                Explore alternativas e descubra onde a partida mudou.
              </p>
            </div>
          </li>
        </ol>
        <div className='mt-4 flex items-start gap-2 border-t border-edge-soft pt-4 text-xs text-ink-faint'>
          <ShieldCheck
            size={14}
            strokeWidth={2}
            className='mt-0.5 shrink-0'
            aria-hidden='true'
          />
          <span>
            Toda a análise acontece localmente — seu PGN não sai do seu
            computador.
          </span>
        </div>
      </div>
    </aside>
  )
}
