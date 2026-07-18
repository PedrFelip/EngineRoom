import { formatEngineTag } from '../lib/engine-tag'
import { resultLabel } from '../lib/pgn'
import type { GameSummary } from '../types'

interface Props {
  games: GameSummary[]
  onOpen: (id: number) => void
  onDelete: (id: number) => void
  onReanalyze: (id: number) => void
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
  })
}

export default function ReviewedGamesList({
  games,
  onOpen,
  onDelete,
  onReanalyze,
}: Props) {
  return (
    <section className='w-full min-w-0 flex-1'>
      <h2 className='mb-3 text-sm font-semibold tracking-wide text-ink-dim uppercase'>
        Partidas analisadas
      </h2>
      <ul className='@container flex max-h-[26rem] flex-col gap-2 overflow-y-auto pr-1 md:max-h-[32rem] xl:max-h-[40rem]'>
        {games.map((g) => (
          <li
            key={g.id}
            className='relative overflow-hidden rounded-xl border border-edge bg-panel/80 transition hover:border-brand/60 hover:bg-panel-2'
          >
            <button
              type='button'
              onClick={() => onOpen(g.id)}
              className='absolute inset-0 z-20 h-full w-full cursor-pointer'
              aria-label={`Abrir partida: ${g.white} vs ${g.black}`}
            />
            <div className='flex w-full items-center gap-2 px-3 py-2.5 text-left @sm:gap-3 @sm:px-4 @sm:py-3'>
              <div className='min-w-0 flex-1'>
                <div className='flex items-baseline gap-x-2 text-sm'>
                  <span className='truncate font-semibold text-ink'>
                    {g.white}
                  </span>
                  <span className='text-ink-faint shrink-0'>vs</span>
                  <span className='truncate font-semibold text-ink'>
                    {g.black}
                  </span>
                  <span className='text-ink-dim shrink-0'>·</span>
                  <span className='text-good shrink-0'>
                    {resultLabel(g.result)}
                  </span>
                </div>
                <div className='mt-0.5 flex flex-wrap items-center gap-x-2 text-xs text-ink-faint'>
                  <span className='whitespace-nowrap'>
                    prec. {Math.round(g.accuracyWhite)}% /{' '}
                    {Math.round(g.accuracyBlack)}%
                  </span>
                  <span className='hidden @sm:inline text-ink-faint/60'>·</span>
                  <span className='hidden @sm:inline whitespace-nowrap'>
                    {Math.ceil(g.plies / 2)} lances
                  </span>
                  <span className='hidden @md:inline text-ink-faint/60'>·</span>
                  <span className='hidden @md:inline whitespace-nowrap'>
                    {tierLabel(g)}
                  </span>
                  <span className='hidden @sm:inline text-ink-faint/60'>·</span>
                  <span className='hidden @sm:inline whitespace-nowrap'>
                    {formatDate(g.createdAt)}
                  </span>
                </div>
              </div>

              <div className='relative z-30 flex shrink-0 items-center gap-1'>
                <button
                  type='button'
                  onClick={(e) => {
                    e.stopPropagation()
                    onReanalyze(g.id)
                  }}
                  className='rounded-lg p-1.5 text-ink-dim transition hover:bg-panel-3 hover:text-ink'
                  aria-label='Reanalisar com outras configurações'
                  title='Reanalisar com outras configurações'
                >
                  <svg
                    width='15'
                    height='15'
                    viewBox='0 0 24 24'
                    fill='none'
                    stroke='currentColor'
                    strokeWidth='2'
                    strokeLinecap='round'
                    strokeLinejoin='round'
                    aria-hidden='true'
                  >
                    <polyline points='23 4 23 10 17 10' />
                    <path d='M20.49 15a9 9 0 1 1-2.12-9.36L23 10' />
                  </svg>
                </button>
                <button
                  type='button'
                  onClick={(e) => {
                    e.stopPropagation()
                    onDelete(g.id)
                  }}
                  className='rounded-lg p-1.5 text-ink-dim transition hover:bg-blunder/15 hover:text-blunder'
                  aria-label='Excluir do histórico'
                  title='Excluir do histórico'
                >
                  <svg
                    width='15'
                    height='15'
                    viewBox='0 0 24 24'
                    fill='none'
                    stroke='currentColor'
                    strokeWidth='2'
                    strokeLinecap='round'
                    strokeLinejoin='round'
                    aria-hidden='true'
                  >
                    <polyline points='3 6 5 6 21 6' />
                    <path d='M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2' />
                  </svg>
                </button>
              </div>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}
