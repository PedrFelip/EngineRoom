import { FileUp, ShieldCheck, SlidersHorizontal, Sparkles } from 'lucide-react'

export default function HowItWorks() {
  return (
    <aside className='w-full min-w-0 flex-1'>
      <p className='section-kicker mb-2'>Como funciona</p>
      <h2 className='mb-3 text-lg font-semibold tracking-tight text-ink'>
        Da partida ao padrão.
      </h2>
      <div className='analysis-surface surface-glass elev-card rounded-2xl border border-edge p-5'>
        <ol className='flex flex-col gap-4'>
          <li className='flex gap-3'>
            <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 text-brand'>
              <FileUp size={14} strokeWidth={2.2} aria-hidden='true' />
            </span>
            <div>
              <p className='text-sm font-semibold text-ink'>Importe um PGN</p>
              <p className='mt-0.5 text-xs text-ink-dim'>
                Arraste um arquivo .pgn ou cole a notação diretamente.
              </p>
            </div>
          </li>
          <li className='flex gap-3'>
            <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 text-brand'>
              <SlidersHorizontal
                size={14}
                strokeWidth={2.2}
                aria-hidden='true'
              />
            </span>
            <div>
              <p className='text-sm font-semibold text-ink'>Ajuste a engine</p>
              <p className='mt-0.5 text-xs text-ink-dim'>
                Escolha profundidade, tempo por lance e linhas candidatas.
              </p>
            </div>
          </li>
          <li className='flex gap-3'>
            <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 text-brand'>
              <Sparkles size={14} strokeWidth={2.2} aria-hidden='true' />
            </span>
            <div>
              <p className='text-sm font-semibold text-ink'>Receba a revisão</p>
              <p className='mt-0.5 text-xs text-ink-dim'>
                Cada lance é classificado com precisão e win% estimado.
              </p>
            </div>
          </li>
        </ol>
        <div className='mt-5 flex items-start gap-2 border-t border-edge-soft pt-4 text-xs text-ink-faint'>
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
