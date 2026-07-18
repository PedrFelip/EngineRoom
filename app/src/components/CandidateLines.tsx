import type { PvLine } from '../types'
import { formatEval } from '../lib/scoring'
import { cpToMate } from '../lib/eval-label'

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
      <div className='mb-1.5 px-1 text-[11px] font-semibold uppercase tracking-wide text-ink-faint'>
        Linhas candidatas
      </div>
      <div className='flex flex-col gap-0.5'>
        {lines.map((l) => {
          const active = l.multipv === selectedMultipv
          const mate = cpToMate(l.cp)
          return (
            <button
              key={l.multipv}
              type='button'
              onClick={() => onSelect(l.multipv)}
              className={`flex items-center gap-2 rounded-md px-2 py-1 text-left text-sm transition ${
                active
                  ? 'bg-brand/15 ring-1 ring-brand/40'
                  : 'hover:bg-panel-3/50'
              }`}
            >
              <span className='w-5 text-center font-mono text-xs text-ink-faint'>
                {l.multipv}
              </span>
              <span className='w-16 font-mono font-semibold text-ink'>
                {l.san ?? '—'}
              </span>
              <span className='font-mono text-xs tabular-nums text-ink-dim'>
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
