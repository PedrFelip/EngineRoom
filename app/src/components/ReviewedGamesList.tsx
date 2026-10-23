import { Archive, RotateCcw, Search, Trash2, X } from 'lucide-react'
import { memo, useEffect, useRef, useState } from 'react'
import { formatEngineTag } from '../lib/engine-tag'
import { resultLabel } from '../lib/pgn'
import type { GameSummary } from '../types'
import { Badge } from './ui/badge'

interface Props {
  busy: boolean
  games: GameSummary[]
  total: number
  hasMore: boolean
  loadingMore: boolean
  onOpen: (id: number) => void
  onDelete: (id: number) => Promise<boolean>
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
  busy,
  games,
  total,
  hasMore,
  loadingMore,
  onOpen,
  onDelete,
  onReanalyze,
  onLoadMore,
}: Props) {
  const [query, setQuery] = useState('')
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null)
  const deleteTrigger = useRef<HTMLButtonElement | null>(null)
  const cancelDelete = useRef<HTMLButtonElement | null>(null)
  const list = useRef<HTMLElement | null>(null)
  const [deleteError, setDeleteError] = useState(false)
  useEffect(() => {
    if (confirmDelete !== null) cancelDelete.current?.focus()
  }, [confirmDelete])
  function dismissDelete(): void {
    setConfirmDelete(null)
    setDeleteError(false)
    deleteTrigger.current?.focus()
  }
  const normalize = (value: string) =>
    value
      .normalize('NFD')
      .replace(/[\u0300-\u036f]/g, '')
      .toLocaleLowerCase('pt-BR')
  const normalizedQuery = normalize(query.trim())
  const filteredGames = games.filter((game) =>
    normalize(`${game.white} ${game.black}`).includes(normalizedQuery),
  )

  return (
    <section
      ref={list}
      tabIndex={-1}
      aria-label='Partidas analisadas'
      id='reviewed-games'
      className='@container w-full min-w-0 flex-1'
    >
      <div className='home-surface overflow-hidden'>
        <header className='flex items-center justify-between gap-3 border-b border-border/70 px-3 py-2.5 @sm:px-4'>
          <h2 className='flex min-w-0 items-center gap-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase'>
            <Archive size={13} strokeWidth={2.2} aria-hidden='true' />
            Partidas analisadas
          </h2>
          <Badge variant='outline' className='font-mono tabular-nums'>
            {total}
          </Badge>
        </header>
        <div className='border-b border-edge-soft p-3'>
          <label className='flex items-center gap-2 rounded-xl border border-edge bg-panel-2/50 px-3 focus-within:ring-2 focus-within:ring-brand'>
            <Search size={16} className='text-ink-dim' aria-hidden='true' />
            <input
              type='search'
              aria-label='Buscar jogadores nas partidas carregadas'
              placeholder='Buscar por jogador'
              value={query}
              onChange={(event) => {
                setQuery(event.target.value)
                setConfirmDelete(null)
                setDeleteError(false)
              }}
              className='h-11 min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-faint'
            />
          </label>
          <p className='mt-2 text-xs text-ink-dim'>
            {query
              ? `${filteredGames.length} resultado(s) nas ${games.length} partidas carregadas`
              : `${games.length} de ${total} partidas · Mais recentes primeiro`}
          </p>
        </div>
        {filteredGames.length === 0 && (
          <p className='px-5 py-10 text-center text-sm text-ink-dim'>
            Nenhuma partida encontrada.{' '}
            {hasMore
              ? 'Carregue mais partidas para ampliar a busca.'
              : 'Tente outro nome.'}
          </p>
        )}
        <ul className='divide-y divide-border/70'>
          {filteredGames.map((g) => (
            <li
              key={g.id}
              className='group relative bg-card/30 transition-colors hover:bg-accent/55'
            >
              <button
                type='button'
                disabled={busy}
                onClick={() => onOpen(g.id)}
                className='absolute inset-0 z-10 h-full w-full cursor-pointer outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring'
                aria-label={`Abrir partida: ${g.white} vs ${g.black}`}
              />
              <div className='relative flex w-full flex-col gap-1 px-3 py-2 text-left @sm:px-4'>
                <div className='min-w-0 flex-1'>
                  <div className='grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 text-sm'>
                    <div className='flex min-w-0 flex-col gap-1 @md:flex-row @md:items-center @md:gap-1.5'>
                      <PlayerName name={g.white} color='white' />
                      <span className='hidden shrink-0 text-[10px] font-medium text-muted-foreground @md:inline'>
                        vs
                      </span>
                      <PlayerName name={g.black} color='black' />
                    </div>
                    <Badge
                      variant='outline'
                      className='row-span-2 shrink-0 border-border/70 bg-transparent text-[10px] text-muted-foreground'
                    >
                      {resultLabel(g.result)}
                    </Badge>
                  </div>
                </div>

                <div className='flex items-center justify-between gap-2'>
                  <div className='flex min-w-0 flex-wrap items-center gap-x-1.5 text-[11px] text-muted-foreground'>
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
                  <div className='relative z-20 flex shrink-0 items-center gap-0.5'>
                    <button
                      type='button'
                      disabled={busy}
                      onClick={(e) => {
                        e.stopPropagation()
                        onReanalyze(g.id)
                      }}
                      className='rounded-[calc(var(--radius)-2px)] p-2.5 text-muted-foreground transition-colors hover:bg-card hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring'
                      aria-label={`Reanalisar ${g.white} vs ${g.black} com outras configurações`}
                      title='Reanalisar com outras configurações'
                    >
                      <RotateCcw size={15} strokeWidth={2} aria-hidden='true' />
                    </button>
                    <button
                      type='button'
                      disabled={busy}
                      onClick={(e) => {
                        e.stopPropagation()
                        deleteTrigger.current = e.currentTarget
                        setDeleteError(false)
                        setConfirmDelete(g.id)
                      }}
                      className='rounded-[calc(var(--radius)-2px)] p-2.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring'
                      aria-label={`Excluir ${g.white} vs ${g.black} do histórico`}
                      title='Excluir do histórico'
                    >
                      <Trash2 size={15} strokeWidth={2} aria-hidden='true' />
                    </button>
                  </div>
                </div>
              </div>
              {confirmDelete === g.id && (
                <div
                  role='alertdialog'
                  aria-labelledby={`delete-title-${g.id}`}
                  aria-describedby={`delete-description-${g.id}`}
                  onKeyDown={(event) => {
                    if (event.key === 'Escape' && !busy) {
                      event.preventDefault()
                      dismissDelete()
                    }
                  }}
                  className='relative z-20 mx-4 mb-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-blunder/25 bg-panel p-3'
                >
                  <div>
                    <p id={`delete-title-${g.id}`} className='text-xs text-ink'>
                      Excluir esta revisão do histórico?
                    </p>
                    <p
                      id={`delete-description-${g.id}`}
                      className='text-xs text-ink-dim'
                    >
                      {g.white} vs {g.black}
                    </p>
                    {deleteError && (
                      <p role='alert' className='text-xs text-blunder'>
                        Não foi possível excluir. Tente novamente.
                      </p>
                    )}
                  </div>
                  <div className='flex items-center gap-2'>
                    <button
                      type='button'
                      ref={cancelDelete}
                      disabled={busy}
                      onClick={dismissDelete}
                      className='inline-flex items-center gap-1 rounded-lg px-3 py-2 text-xs text-ink-dim hover:bg-panel-2'
                    >
                      <X size={12} aria-hidden='true' />
                      Cancelar
                    </button>
                    <button
                      type='button'
                      disabled={busy}
                      onClick={async () => {
                        const removed = await onDelete(g.id)
                        if (removed) {
                          setConfirmDelete(null)
                          requestAnimationFrame(() => {
                            const target =
                              list.current ??
                              document.getElementById('home-page-title')
                            target?.focus()
                          })
                        } else {
                          setDeleteError(true)
                        }
                      }}
                      className='rounded-lg bg-blunder/10 px-3 py-2 text-xs font-medium text-blunder hover:bg-blunder/20'
                    >
                      Excluir revisão
                    </button>
                  </div>
                </div>
              )}
            </li>
          ))}
        </ul>
        {hasMore ? (
          <div className='border-t border-border/70 p-2.5 text-center'>
            <button
              type='button'
              onClick={onLoadMore}
              disabled={busy || loadingMore}
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

function PlayerName({
  color,
  name,
}: {
  color: 'white' | 'black'
  name: string
}) {
  return (
    <span className='flex min-w-0 items-center gap-1.5'>
      <PlayerDot color={color} />
      <span className='min-w-0 truncate font-semibold text-foreground'>
        {name}
      </span>
    </span>
  )
}
