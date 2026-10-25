import {
  ArrowDown,
  BookmarkCheck,
  Check,
  ChevronRight,
  CircleAlert,
  Clock3,
  LoaderCircle,
  type LucideIcon,
  RefreshCw,
  Rocket,
  Search,
  Sun,
  Swords,
  Timer,
  UserRound,
  X,
  Zap,
} from 'lucide-react'
import { useId, useState } from 'react'
import { chessComGamePerspective } from '../lib/chess-com-account'
import type { RemoteGame } from '../lib/chess-com-import'
import { useChessComImport } from '../lib/use-chess-com-import'
import { Badge } from './ui/badge'
import { Button } from './ui/button'

const timeClasses: Record<string, string> = {
  bullet: 'Bullet',
  blitz: 'Blitz',
  rapid: 'Rápida',
  daily: 'Diária',
}
const timeClassIcons: Record<string, LucideIcon> = {
  bullet: Rocket,
  blitz: Zap,
  rapid: Timer,
  daily: Sun,
}
const dateFormat = new Intl.DateTimeFormat('pt-BR', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
})

export default function ChessComImporter({
  onImport,
  active = true,
}: {
  active?: boolean
  onImport: (pgn: string) => void
}) {
  const {
    state,
    savedUsername,
    setUsername,
    search,
    cancel,
    select,
    forgetUsername,
  } = useChessComImport(onImport, active)
  const id = useId()
  const [filter, setFilter] = useState<string | null>(null)
  const games = filter
    ? state.items.filter((game) => game.timeClass === filter)
    : state.items
  const selected = state.items.find((game) => game.url === state.selectedUrl)
  const refreshing =
    state.searchedUsername === state.username.trim().toLowerCase() &&
    state.result !== null

  return (
    <section
      aria-label='Importar do Chess.com'
      className='overflow-hidden rounded-xl border border-border bg-card/60 shadow-sm'
    >
      <header className='border-b border-border/70 bg-gradient-to-br from-brand/5 to-transparent p-4'>
        <div className='mb-4 flex items-center gap-3'>
          <div className='min-w-0'>
            <h3 className='text-sm font-semibold text-foreground'>
              Suas partidas, a um clique.
            </h3>
            <p className='mt-0.5 text-xs text-muted-foreground'>
              Selecione uma partida do Chess.com para revisar.
            </p>
          </div>
        </div>

        <form
          onSubmit={(event) => {
            event.preventDefault()
            search()
          }}
        >
          <label htmlFor={`${id}-username`} className='sr-only'>
            Nome de usuário no Chess.com
          </label>
          <div className='flex gap-2'>
            <div className='flex min-w-0 flex-1 items-center gap-2 rounded-[var(--radius)] border border-input bg-card px-3 shadow-sm transition focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/20'>
              <UserRound
                size={15}
                className='shrink-0 text-muted-foreground'
                aria-hidden='true'
              />
              <input
                id={`${id}-username`}
                value={state.username}
                onChange={(event) => {
                  setFilter(null)
                  setUsername(event.target.value)
                }}
                autoCapitalize='none'
                autoCorrect='off'
                spellCheck={false}
                maxLength={50}
                placeholder='Seu nome de usuário'
                className='h-10 w-full min-w-0 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground'
              />
            </div>
            <Button
              type='submit'
              className='h-10 px-3'
              disabled={
                state.loading || state.coolingDown || !state.username.trim()
              }
            >
              {state.loading ? (
                <LoaderCircle
                  size={15}
                  className='motion-safe:animate-spin'
                  aria-hidden='true'
                />
              ) : refreshing ? (
                <RefreshCw size={15} aria-hidden='true' />
              ) : (
                <Search size={15} aria-hidden='true' />
              )}
              {refreshing ? 'Atualizar' : 'Buscar'}
            </Button>
          </div>
          <div
            hidden={!savedUsername}
            className='mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-2'
          >
            {savedUsername && (
              <span className='inline-flex max-w-full items-center gap-1 rounded-full border border-border bg-card/80 py-0.5 pl-2 pr-0.5 text-[11px] text-muted-foreground'>
                <BookmarkCheck size={12} aria-hidden='true' />
                <button
                  type='button'
                  onClick={() => {
                    setFilter(null)
                    setUsername(savedUsername)
                    search()
                  }}
                  className='min-w-0 truncate rounded px-1 py-0.5 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
                  title='Buscar o usuário salvo'
                >
                  @{savedUsername}
                </button>
                <button
                  type='button'
                  onClick={() => {
                    setFilter(null)
                    forgetUsername()
                  }}
                  aria-label='Esquecer usuário salvo'
                  title='Esquecer usuário salvo'
                  className='rounded-full p-1 transition hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
                >
                  <X size={11} aria-hidden='true' />
                </button>
              </span>
            )}
          </div>
        </form>
      </header>

      <div aria-live='polite' aria-atomic='true'>
        {state.loading && (
          <div className='flex items-center justify-between gap-3 border-b border-border/70 px-4 py-2.5'>
            <p className='flex items-center gap-2 text-xs text-muted-foreground'>
              <LoaderCircle
                size={13}
                className='motion-safe:animate-spin'
                aria-hidden='true'
              />
              {state.items.length
                ? 'Buscando partidas anteriores…'
                : `Buscando partidas de @${state.username.trim()}…`}
            </p>
            <Button size='sm' variant='ghost' onClick={cancel}>
              Cancelar
            </Button>
          </div>
        )}
        {(state.error || state.result?.warning) && (
          <div
            role={state.error ? 'alert' : 'status'}
            className={`m-3 flex items-start gap-2 rounded-lg border px-3 py-2.5 text-xs ${state.error ? 'border-destructive/20 bg-destructive/5 text-destructive' : 'border-mistake/25 bg-mistake/5 text-foreground'}`}
          >
            <CircleAlert
              size={14}
              className='mt-0.5 shrink-0'
              aria-hidden='true'
            />
            <p>
              {state.error ??
                `Resultados parciais: ${state.result?.warning?.message}`}
            </p>
          </div>
        )}
        {state.coolingDown && (
          <p className='px-4 py-2 text-xs text-muted-foreground'>
            O Chess.com pediu uma pausa. A busca será liberada automaticamente.
          </p>
        )}
      </div>

      {state.loading && state.items.length === 0 && (
        <div aria-hidden='true' className='space-y-3 p-4'>
          {[1, 2, 3].map((row) => (
            <div
              key={row}
              className='flex items-center gap-3 motion-safe:animate-pulse'
            >
              <div className='size-9 rounded-lg bg-muted' />
              <div className='flex-1 space-y-2'>
                <div className='h-3 w-2/3 rounded bg-muted' />
                <div className='h-2.5 w-1/2 rounded bg-muted' />
              </div>
              <div className='h-5 w-14 rounded-full bg-muted' />
            </div>
          ))}
        </div>
      )}

      {state.items.length > 0 ? (
        <>
          <div className='px-4 pb-3 pt-3.5'>
            <div className='mb-2.5 flex items-center justify-between gap-2'>
              <h4 className='text-[11px] font-semibold uppercase tracking-wide text-muted-foreground'>
                Partidas recentes
              </h4>
              <Badge
                variant='outline'
                className='font-mono text-[10px] tabular-nums'
              >
                {state.items.length}
              </Badge>
            </div>
            <fieldset
              aria-label='Filtrar partidas carregadas por ritmo'
              className='flex flex-wrap gap-1'
            >
              {[
                null,
                ...Object.keys(timeClasses).filter((type) =>
                  state.items.some((game) => game.timeClass === type),
                ),
              ].map((type) => (
                <button
                  key={type ?? 'all'}
                  type='button'
                  aria-pressed={filter === type}
                  onClick={() => setFilter(type)}
                  className={`rounded-full px-2.5 py-1 text-[11px] font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${filter === type ? 'bg-primary text-primary-foreground' : 'bg-muted/60 text-muted-foreground hover:bg-accent hover:text-foreground'}`}
                >
                  {type ? timeClasses[type] : 'Todas'}
                </button>
              ))}
            </fieldset>
          </div>
          <fieldset className='min-w-0'>
            <legend className='sr-only'>
              Escolha uma partida para preencher a prévia
            </legend>
            <div className='max-h-72 divide-y divide-border/60 overflow-y-auto border-y border-border/70'>
              {games.map((game) => (
                <GameCard
                  key={game.url}
                  game={game}
                  username={state.searchedUsername ?? ''}
                  selected={game.url === state.selectedUrl}
                  radioName={`${id}-game`}
                  onSelect={() => select(game.url)}
                />
              ))}
              {games.length === 0 && (
                <div className='p-5 text-center text-xs text-muted-foreground'>
                  <p>Nenhuma partida desse ritmo na lista carregada.</p>
                  <Button
                    size='sm'
                    variant='link'
                    onClick={() => setFilter(null)}
                  >
                    Mostrar todas
                  </Button>
                </div>
              )}
            </div>
          </fieldset>
          {selected && (
            <div
              role='status'
              className='flex items-center gap-2 border-b border-border/70 bg-good/5 px-4 py-2.5 text-xs text-foreground'
            >
              <Check
                size={14}
                className='shrink-0 text-good'
                aria-hidden='true'
              />
              <span>
                Partida selecionada. Clique em <strong>Continuar</strong> para
                configurar a análise.
              </span>
            </div>
          )}
        </>
      ) : (
        !state.loading &&
        !state.error && (
          <div className='flex flex-col items-center px-5 py-7 text-center'>
            <Swords
              size={25}
              className='mb-2.5 text-muted-foreground/60'
              aria-hidden='true'
            />
            <p className='text-sm font-medium text-foreground'>
              {state.result
                ? 'Nenhuma partida encontrada'
                : 'Do Chess.com para sua análise'}
            </p>
            <p className='mt-1 max-w-xs text-xs leading-relaxed text-muted-foreground'>
              {state.result
                ? state.cursor
                  ? 'Não há partidas compatíveis nos arquivos consultados. Você pode continuar buscando anteriores.'
                  : 'Não há partidas concluídas de xadrez padrão disponíveis para esse usuário.'
                : 'Busque seu usuário e escolha uma partida. O PGN entra na prévia assim que você selecionar.'}
            </p>
          </div>
        )
      )}

      {state.cursor && (
        <div className='p-2.5 text-center'>
          <Button
            size='sm'
            variant='ghost'
            className='w-full text-muted-foreground hover:text-foreground'
            disabled={state.loading || state.coolingDown}
            onClick={() => search(true)}
          >
            <ArrowDown size={13} aria-hidden='true' />
            Buscar anteriores
          </Button>
        </div>
      )}
      <footer className='border-t border-border/60 bg-muted/20 px-4 py-2.5 text-[10px] leading-relaxed text-muted-foreground'>
        {state.items.length >= 100
          ? '100 partidas carregadas. Atualize a busca para recomeçar.'
          : state.result?.stopReason === 'limit'
            ? 'Limite desta consulta atingido. Continue em “Buscar anteriores”.'
            : 'Somente partidas concluídas de xadrez padrão. A API pode demorar para mostrar partidas recém-terminadas.'}
      </footer>
    </section>
  )
}

function GameCard({
  game,
  username,
  selected,
  radioName,
  onSelect,
}: {
  game: RemoteGame
  username: string
  selected: boolean
  radioName: string
  onSelect: () => void
}) {
  const TimeClassIcon = timeClassIcons[game.timeClass ?? ''] ?? Clock3
  const perspective = chessComGamePerspective(game, username)
  const title =
    perspective.opponent?.username ??
    `${game.white.username} × ${game.black.username}`
  return (
    <label
      className={`group relative flex cursor-pointer items-center gap-3 px-4 py-3 text-left transition-colors focus-within:z-10 focus-within:ring-2 focus-within:ring-inset focus-within:ring-ring ${selected ? 'bg-primary/5' : 'hover:bg-accent/50'}`}
    >
      <input
        type='radio'
        name={radioName}
        value={game.url}
        checked={selected}
        onChange={onSelect}
        aria-label={`Selecionar partida: ${title}, ${perspective.label}, ${dateFormat.format(new Date(game.endTime * 1000))}`}
        className='sr-only'
      />
      <span
        aria-hidden='true'
        className={`flex size-9 shrink-0 items-center justify-center rounded-lg border ${selected ? 'border-primary/20 bg-primary text-primary-foreground' : 'border-border bg-muted/40 text-muted-foreground'}`}
      >
        {selected ? <Check size={16} /> : <TimeClassIcon size={15} />}
      </span>
      <span className='min-w-0 flex-1'>
        <span className='flex min-w-0 items-baseline gap-1.5'>
          {perspective.opponent && (
            <span className='text-[10px] text-muted-foreground'>vs</span>
          )}
          <span
            className='truncate text-sm font-semibold text-foreground'
            title={title}
          >
            {title}
          </span>
          {perspective.opponent?.rating != null && (
            <span className='shrink-0 font-mono text-[10px] text-muted-foreground'>
              {perspective.opponent.rating}
            </span>
          )}
        </span>
        <span className='mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-muted-foreground'>
          <span className='inline-flex items-center gap-1'>
            <TimeClassIcon size={10} aria-hidden='true' />
            {game.timeClass
              ? (timeClasses[game.timeClass] ?? game.timeClass)
              : 'Ritmo não informado'}
          </span>
          {perspective.color && (
            <span className='inline-flex items-center gap-1'>
              <span
                aria-hidden='true'
                className='size-1.5 rounded-full ring-1 ring-border'
                style={{
                  backgroundColor:
                    perspective.color === 'white'
                      ? 'var(--evalbar-side-white)'
                      : 'var(--evalbar-side-black)',
                }}
              />
              {perspective.color === 'white' ? 'Brancas' : 'Pretas'}
            </span>
          )}
          <span>{dateFormat.format(new Date(game.endTime * 1000))}</span>
        </span>
      </span>
      <span
        className={`shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-medium ${perspective.outcome === 'win' ? 'border-good/20 bg-good/10 text-good' : perspective.outcome === 'loss' ? 'border-destructive/15 bg-destructive/5 text-destructive' : 'border-border bg-muted/40 text-muted-foreground'}`}
      >
        {perspective.label}
      </span>
      <ChevronRight
        size={13}
        aria-hidden='true'
        className={`hidden shrink-0 sm:block ${selected ? 'text-foreground' : 'text-muted-foreground/40 group-hover:text-muted-foreground'}`}
      />
    </label>
  )
}
