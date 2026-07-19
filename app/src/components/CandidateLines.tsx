import { useEffect, useState } from 'react'
import { cpToMate } from '../lib/eval-label'
import { formatEval } from '../lib/scoring'
import type { PvLine } from '../types'

interface CandidateLinesProps {
  lines: PvLine[]
  selectedMultipv: number
  onSelect: (multipv: number) => void
  /** Indicador opcional de profundidade ao vivo (mostra "AO VIVO d{N}"). */
  liveDepth?: number | null
}

/**
 * Painel de linhas candidatas como modal dinâmico: um resumo compacto fica
 * sempre visível (melhor lance + eval), e clicar abre um overlay centralizado
 * com todas as variações — atualizadas em tempo real conforme o refino corre.
 *
 * A ideia é não ocupar espaço vertical permanente e dar foco às variações só
 * quando o usuário quer explorá-las.
 */
export default function CandidateLines({
  lines,
  selectedMultipv,
  onSelect,
  liveDepth,
}: CandidateLinesProps) {
  const [open, setOpen] = useState(false)

  // Esc fecha o modal.
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open])

  if (lines.length === 0) return null

  const best = lines.find((l) => l.multipv === 1) ?? lines[0]
  const bestMate = cpToMate(best?.cp ?? 0)
  const bestLabel =
    bestMate !== null ? `M${Math.abs(bestMate)}` : formatEval(best?.cp ?? 0)

  return (
    <>
      <button
        type='button'
        onClick={() => setOpen(true)}
        className='flex w-full items-center justify-between gap-2 rounded-xl border border-edge bg-panel-2/60 px-3 py-2 text-sm transition hover:bg-panel-3/40'
        title='Ver linhas candidatas'
      >
        <span className='flex items-center gap-2'>
          <span className='text-[11px] font-semibold uppercase tracking-wide text-ink-faint'>
            Linhas
          </span>
          <span className='font-mono font-semibold text-ink'>
            {best?.san ?? '—'}
          </span>
          <span className='font-mono text-xs tabular-nums text-ink-dim'>
            {bestLabel}
          </span>
        </span>
        <span className='flex items-center gap-2 text-[11px] text-ink-faint'>
          {liveDepth ? (
            <span className='font-mono text-brand'>d{liveDepth}</span>
          ) : null}
          <span>{lines.length} variações ▾</span>
        </span>
      </button>

      {open && (
        <div
          className='fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm'
          onClick={() => setOpen(false)}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            className='w-full max-w-md rounded-2xl border border-edge bg-panel-2 p-3 shadow-2xl'
          >
            <div className='mb-2 flex items-center justify-between px-1'>
              <div className='flex items-center gap-2'>
                <span className='text-sm font-semibold text-ink'>
                  Linhas candidatas
                </span>
                {liveDepth ? (
                  <span className='rounded-md bg-brand/15 px-1.5 py-0.5 font-mono text-[11px] text-brand ring-1 ring-brand/30'>
                    AO VIVO · d{liveDepth}
                  </span>
                ) : null}
              </div>
              <button
                type='button'
                onClick={() => setOpen(false)}
                className='rounded-md px-2 py-0.5 text-ink-faint transition hover:bg-panel-3/60 hover:text-ink'
                aria-label='Fechar'
              >
                ✕
              </button>
            </div>
            <div className='mb-1.5 px-1 text-[11px] text-ink-faint'>
              melhor em destaque · variações alternativas mais suaves
            </div>
            <div className='flex max-h-[60vh] flex-col gap-0.5 overflow-y-auto'>
              {lines.map((l) => {
                const active = l.multipv === selectedMultipv
                const mate = cpToMate(l.cp)
                // Hierarquia visual: principal opaca, próximas duas médias, restante baixa.
                const opacityClass =
                  l.multipv === 1
                    ? ''
                    : l.multipv <= 3
                      ? 'opacity-80'
                      : 'opacity-50'
                const textClass =
                  l.multipv === 1 ? 'text-ink font-semibold' : 'text-ink-dim'
                return (
                  <button
                    key={l.multipv}
                    type='button'
                    onClick={() => {
                      onSelect(l.multipv)
                      setOpen(false)
                    }}
                    className={`flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition ${opacityClass} ${
                      active
                        ? 'bg-brand/15 ring-1 ring-brand/40'
                        : l.multipv === 1
                          ? 'bg-panel-3/40 hover:bg-panel-3/60'
                          : 'hover:bg-panel-3/50'
                    }`}
                  >
                    <span className='w-6 text-center font-mono text-xs text-ink-faint'>
                      {l.multipv}
                    </span>
                    <span className={`w-20 font-mono ${textClass}`}>
                      {l.san ?? '—'}
                    </span>
                    <span
                      className={`font-mono text-xs tabular-nums ${textClass}`}
                    >
                      {formatEval(l.cp)}
                    </span>
                    {mate === null && (
                      <span className='ml-auto font-mono text-xs tabular-nums text-ink-faint'>
                        {l.winPct.toFixed(1)}%
                      </span>
                    )}
                  </button>
                )
              })}
            </div>
          </div>
        </div>
      )}
    </>
  )
}
