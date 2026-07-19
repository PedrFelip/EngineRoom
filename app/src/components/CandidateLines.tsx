import { cpToMate } from '../lib/eval-label'
import { formatEval } from '../lib/scoring'
import type { PvLine } from '../types'

interface CandidateLinesProps {
  lines: PvLine[]
  selectedMultipv: number
  onSelect: (multipv: number) => void
}

export default function CandidateLines({
  lines,
  selectedMultipv,
  onSelect,
}: CandidateLinesProps) {
  if (lines.length === 0) return null
  return (
    <div className='rounded-xl border border-edge bg-panel-2/60 p-2'>
      <div className='mb-1.5 flex items-center justify-between px-1'>
        <span className='text-[11px] font-semibold uppercase tracking-wide text-ink-faint'>
          Linhas candidatas
        </span>
        <span className='text-[11px] text-ink-faint'>melhor em destaque</span>
      </div>
      <div className='flex flex-col gap-0.5'>
        {lines.map((l) => {
          const active = l.multipv === selectedMultipv
          const mate = cpToMate(l.cp)
          // Hierarquia visual: principal opaca, próximas duas médias, restante baixa.
          const opacityClass =
            l.multipv === 1 ? '' : l.multipv <= 3 ? 'opacity-80' : 'opacity-50'
          const textClass =
            l.multipv === 1 ? 'text-ink font-semibold' : 'text-ink-dim'
          return (
            <button
              key={l.multipv}
              type='button'
              onClick={() => onSelect(l.multipv)}
              className={`flex items-center gap-2 rounded-md px-2 py-1 text-left text-sm transition ${opacityClass} ${
                active
                  ? 'bg-brand/15 ring-1 ring-brand/40'
                  : l.multipv === 1
                    ? 'bg-panel-3/40 hover:bg-panel-3/60'
                    : 'hover:bg-panel-3/50'
              }`}
            >
              <span className='w-5 text-center font-mono text-xs text-ink-faint'>
                {l.multipv}
              </span>
              <span className={`w-16 font-mono ${textClass}`}>
                {l.san ?? '—'}
              </span>
              <span className={`font-mono text-xs tabular-nums ${textClass}`}>
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
  )
}
