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

export default function CandidateLines({
  lines,
  selectedMultipv,
  onSelect,
  fen,
  onExplore,
}: CandidateLinesProps) {
  if (lines.length === 0) return null
  return (
    <div className='rounded-xl border border-border bg-card/60 p-2 shadow-sm'>
      <div className='mb-2 flex items-center gap-1.5 px-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground'>
        <ListTree size={12} strokeWidth={2.2} aria-hidden='true' />
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
              className={`flex min-h-8 items-center gap-2 rounded-[calc(var(--radius)-3px)] px-2 text-left text-sm transition-colors ${
                active
                  ? 'bg-accent text-accent-foreground ring-1 ring-border'
                  : 'hover:bg-accent/70'
              }`}
            >
              <span className='flex size-5 shrink-0 items-center justify-center rounded-sm bg-muted/70 font-mono text-[10px] text-muted-foreground'>
                {l.multipv}
              </span>
              <span className='w-16 truncate font-mono font-semibold text-ink'>
                {l.san ?? '—'}
              </span>
              <span className='font-mono text-xs tabular-nums text-muted-foreground'>
                {formatEval(l.cp)}
              </span>
              {mate === null && (
                <span className='ml-auto font-mono text-xs tabular-nums text-muted-foreground'>
                  {l.winPct.toFixed(1)}%
                </span>
              )}
            </button>
          )
        })}
      </div>
      <div className='mt-2 border-t border-border px-1 pt-2'>
        <div className='flex flex-wrap gap-1 text-xs text-ink-dim'>
          {formatPv(
            fen,
            lines.find((line) => line.multipv === selectedMultipv)?.pv ?? [],
          ).map((move) => (
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
          onClick={() =>
            onExplore(
              lines.find((line) => line.multipv === selectedMultipv)?.pv ?? [],
            )
          }
          className='mt-2 flex w-full items-center justify-center gap-1.5 rounded-md border border-edge bg-panel-2 px-3 py-1.5 text-xs font-medium text-ink-dim transition hover:border-brand/40 hover:bg-brand/10 hover:text-brand'
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
