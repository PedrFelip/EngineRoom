import { Archive, RotateCcw, Trash2 } from 'lucide-react'
import { memo } from 'react'
import { formatEngineTag } from '../lib/engine-tag'
import { resultLabel } from '../lib/pgn'
import type { GameSummary } from '../types'
import { Badge } from './ui/badge'

interface Props {
  games: GameSummary[]
  total: number
  hasMore: boolean
  loadingMore: boolean
  onOpen: (id: number) => void
  onDelete: (id: number) => void
  onReanalyze: (id: number) => void
  onLoadMore: () => void
}

/** "2026-07-17 20:00:00" (UTC do SQLite) → "17/07 17:00" (local). */
function formatDate(createdAt: string): string {
  const d = new Date(`${createdAt.replace(' ', 'T')}Z`)
  if (Number.isNaN(d.getTime())) return createdAt
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mi = String(d.getMinutes()).padStart(2, '0')
  return `${dd}/${mm} ${hh}:${mi}`
}

function tierLabel(game: GameSummary): string {
  return formatEngineTag({
    mode: game.mode ?? 'depth',
    depth: game.depth,
    engineTier: game.engineTier,
    analysisKind: game.analysisKind,
  })
}

const ReviewedGamesList = memo(function ReviewedGamesList({
  games,
  total,
  hasMore,
  loadingMore,
  onOpen,
  onDelete,
  onReanalyze,
  onLoadMore,
}: Props) {
  return (
    <section className='@container w-full min-w-0 flex-1'>
      <div className='overflow-hidden rounded-xl border border-border bg-card/60 shadow-sm'>
        <header className='flex items-center justify-between gap-3 border-b border-border/70 px-3 py-2.5 @sm:px-4'>
          <h2 className='flex min-w-0 items-center gap-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase'>
            <Archive size={13} strokeWidth={2.2} aria-hidden='true' />
            Partidas analisadas
          </h2>
          <Badge variant='outline' className='font-mono tabular-nums'>
            {total}
          </Badge>
        </header>
        <ul className='max-h-[25rem] divide-y divide-border/70 overflow-y-auto md:max-h-[31rem] xl:max-h-[39rem]'>
          {games.map((g) => (
            <li
              key={g.id}
              className='group relative bg-card/30 transition-colors hover:bg-accent/55'
            >
              <button
                type='button'
                onClick={() => onOpen(g.id)}
                className='absolute inset-0 z-10 h-full w-full cursor-pointer outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring'
                aria-label={`Abrir partida: ${g.white} vs ${g.black}`}
              />
              <div className='relative flex w-full items-center gap-2.5 px-3 py-3 text-left @sm:gap-3 @sm:px-4'>
                <div className='min-w-0 flex-1'>
                  <div className='flex min-w-0 items-center gap-1.5 text-sm'>
                    <PlayerDot color='white' />
                    <span className='min-w-0 truncate font-semibold text-foreground'>
                      {g.white}
                    </span>
                    <span className='shrink-0 text-[10px] font-medium text-muted-foreground'>
                      vs
                    </span>
                    <PlayerDot color='black' />
                    <span className='min-w-0 truncate font-semibold text-foreground'>
                      {g.black}
                    </span>
                    <Badge
                      variant='outline'
                      className='ml-auto shrink-0 border-border/70 bg-transparent text-[10px] text-muted-foreground'
                    >
                      {resultLabel(g.result)}
                    </Badge>
                  </div>
                  <div className='mt-1 flex flex-wrap items-center gap-x-1.5 text-[11px] text-muted-foreground'>
                    <span className='whitespace-nowrap'>
                      precisão {Math.round(g.accuracyWhite)}% /{' '}
                      {Math.round(g.accuracyBlack)}%
                    </span>
                    <span className='text-border'>·</span>
                    <span className='whitespace-nowrap'>
                      {Math.ceil(g.plies / 2)} lances
                    </span>
                    <span className='hidden text-border @md:inline'>·</span>
                    <span className='hidden whitespace-nowrap @md:inline'>
                      {tierLabel(g)}
                    </span>
                    <span className='hidden text-border @sm:inline'>·</span>
                    <span className='hidden whitespace-nowrap @sm:inline'>
                      {formatDate(g.createdAt)}
                    </span>
                  </div>
                </div>

                <div className='relative z-20 flex shrink-0 items-center gap-0.5'>
                  <button
                    type='button'
                    onClick={(e) => {
                      e.stopPropagation()
                      onReanalyze(g.id)
                    }}
                    className='rounded-[calc(var(--radius)-2px)] p-1.5 text-muted-foreground transition-colors hover:bg-card hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring'
                    aria-label='Reanalisar com outras configurações'
                    title='Reanalisar com outras configurações'
                  >
                    <RotateCcw size={15} strokeWidth={2} aria-hidden='true' />
                  </button>
                  <button
                    type='button'
                    onClick={(e) => {
                      e.stopPropagation()
                      onDelete(g.id)
                    }}
                    className='rounded-[calc(var(--radius)-2px)] p-1.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring'
                    aria-label='Excluir do histórico'
                    title='Excluir do histórico'
                  >
                    <Trash2 size={15} strokeWidth={2} aria-hidden='true' />
                  </button>
                </div>
              </div>
            </li>
          ))}
        </ul>
        {hasMore ? (
          <div className='border-t border-border/70 p-2.5 text-center'>
            <button
              type='button'
              onClick={onLoadMore}
              disabled={loadingMore}
              className='rounded-[var(--radius)] px-3 py-1.5 text-xs font-semibold text-brand transition-colors hover:bg-brand/10 disabled:cursor-wait disabled:opacity-60'
            >
              {loadingMore ? 'Carregando…' : 'Carregar mais'}
            </button>
          </div>
        ) : null}
      </div>
    </section>
  )
})

export default ReviewedGamesList

function PlayerDot({ color }: { color: 'white' | 'black' }) {
  return (
    <span
      className='size-2 shrink-0 rounded-full ring-1 ring-border'
      style={{
        backgroundColor:
          color === 'white'
            ? 'var(--evalbar-side-white)'
            : 'var(--evalbar-side-black)',
      }}
      aria-hidden='true'
    />
  )
}
