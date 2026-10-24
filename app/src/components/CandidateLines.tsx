import { Chess } from 'chess.js'
import { ListTree, Play } from 'lucide-react'
import { cpToMate } from '../lib/eval-label'
import { formatEval } from '../lib/scoring'
import type { PvLine } from '../types'

interface CandidateLinesProps {
  lines: PvLine[]
  selectedMultipv: number
  onSelect: (multipv: number) => void
  fen: string
  onExplore: (pv: string[]) => void
}

const MAX_VISIBLE_LINES = 3

export default function CandidateLines({
  lines,
  selectedMultipv,
  onSelect,
  fen,
  onExplore,
}: CandidateLinesProps) {
  const visibleLines = lines.slice(0, MAX_VISIBLE_LINES)
  if (visibleLines.length === 0) return null
  const selectedLine =
    visibleLines.find((line) => line.multipv === selectedMultipv) ??
    visibleLines[0]
  return (
    <div className='surface-glass min-w-0 rounded-2xl border border-edge p-3'>
      <div className='mb-2 flex items-center gap-1.5 px-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground'>
        <ListTree size={12} strokeWidth={2.2} aria-hidden='true' />
        Linhas candidatas
      </div>
      <div className='flex flex-col gap-0.5'>
        {visibleLines.map((l) => {
          const active = l.multipv === selectedMultipv
          const mate = cpToMate(l.cp)
          return (
            <button
              key={l.multipv}
              type='button'
              onClick={() => onSelect(l.multipv)}
              aria-pressed={active}
              className={`grid min-h-10 min-w-0 grid-cols-[1.25rem_minmax(0,1fr)_5.5ch_3ch_5.5ch] items-center gap-x-2 gap-y-1 rounded-[calc(var(--radius)-3px)] px-2 py-1 text-left text-sm transition-colors ${
                active
                  ? 'bg-accent text-accent-foreground ring-1 ring-border'
                  : 'hover:bg-accent/70'
              }`}
            >
              <span className='flex size-5 shrink-0 items-center justify-center rounded-md bg-muted/70 font-mono text-[10px] text-muted-foreground'>
                {l.multipv}
              </span>
              <span className='min-w-0 truncate font-mono font-semibold text-ink'>
                {l.san ?? '—'}
              </span>
              <span className='text-right font-mono text-xs tabular-nums text-muted-foreground'>
                {formatEval(l.cp)}
              </span>
              <span className='text-right font-mono text-[10px] text-muted-foreground'>
                {l.depth !== undefined ? `d${l.depth}` : ''}
              </span>
              <span className='text-right font-mono text-xs tabular-nums text-muted-foreground'>
                {mate === null ? `${l.winPct.toFixed(1)}%` : ''}
              </span>
            </button>
          )
        })}
      </div>
      <div className='mt-2 border-t border-border px-1 pt-2'>
        <div className='flex max-h-24 min-w-0 flex-wrap gap-1 overflow-y-auto text-xs text-ink-dim'>
          {formatPv(fen, selectedLine.pv).map((move) => (
            <span
              key={move.key}
              className='rounded bg-muted/70 px-1.5 py-0.5 font-mono'
            >
              {move.san}
            </span>
          ))}
        </div>
        <button
          type='button'
          onClick={() => onExplore(selectedLine.pv)}
          className='mt-2 flex w-full items-center justify-center gap-1.5 rounded-xl border border-edge bg-panel-2 px-3 py-2.5 text-xs font-medium text-ink-dim transition hover:border-brand/40 hover:bg-brand/10 hover:text-brand'
        >
          <Play size={12} aria-hidden='true' />
          Explorar esta linha
        </button>
      </div>
    </div>
  )
}

function formatPv(
  fen: string,
  pv: string[],
): Array<{ key: string; san: string }> {
  try {
    const chess = new Chess(fen)
    let path = ''
    return pv.map((uci) => {
      path += uci
      const move = chess.move({
        from: uci.slice(0, 2),
        to: uci.slice(2, 4),
        promotion: uci[4],
      })
      return { key: path, san: move?.san ?? uci }
    })
  } catch {
    let path = ''
    return pv.map((uci) => {
      path += uci
      return { key: path, san: uci }
    })
  }
}
