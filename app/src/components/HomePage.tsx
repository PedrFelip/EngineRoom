import { useEffect, useMemo, useState } from 'react'
import { deleteGame, getGame, listGames, storedToConfig } from '../lib/games'
import { parsePgn, resultLabel } from '../lib/pgn'
import {
  ENGINE_TIERS,
  type EngineTierId,
  type GameSummary,
  type ReviewConfig,
} from '../types'
import EngineDepthSlider from './EngineDepthSlider'
import PgnImporter from './PgnImporter'
import ReviewedGamesList from './ReviewedGamesList'
import SettingsModal from './SettingsModal'

interface Props {
  onStart: (config: ReviewConfig) => void
}

export default function HomePage({ onStart }: Props) {
  const [pgn, setPgn] = useState('')
  const [tierId, setTierId] = useState<EngineTierId>('balanced')
  const [lines, setLines] = useState(1)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [games, setGames] = useState<GameSummary[]>([])

  const parse = useMemo(() => parsePgn(pgn), [pgn])
  const engine = ENGINE_TIERS.find((t) => t.id === tierId)!
  const canStart = parse.ok && pgn.trim().length > 0

  useEffect(() => {
    let cancelled = false
    listGames()
      .then((g) => !cancelled && setGames(g))
      .catch((e) => console.warn('Falha ao listar partidas analisadas:', e))
    return () => {
      cancelled = true
    }
  }, [])

  const openStored = async (id: number) => {
    const game = await getGame(id).catch((e) => {
      console.warn('Falha ao abrir partida:', e)
      return null
    })
    if (game) onStart(storedToConfig(game))
  }

  const removeStored = async (id: number) => {
    await deleteGame(id).catch((e) =>
      console.warn('Falha ao excluir partida:', e),
    )
    setGames((prev) => prev.filter((g) => g.id !== id))
  }

  const reanalyzeStored = async (id: number) => {
    const game = await getGame(id).catch((e) => {
      console.warn('Falha ao carregar partida:', e)
      return null
    })
    if (game) {
      setPgn(game.pgn)
      window.scrollTo({ top: 0, behavior: 'smooth' })
    }
  }

  const hasGames = games.length > 0

  return (
    <div className='flex min-h-full flex-col items-center px-4 py-10'>
      {/* Brand */}
      <header
        className={`mb-8 flex w-full items-center justify-between ${hasGames ? 'max-w-5xl' : 'max-w-xl'}`}
      >
        <div className='flex items-center gap-2.5'>
          <div className='flex h-9 w-9 items-center justify-center rounded-lg bg-brand text-bg'>
            <svg width='20' height='20' viewBox='0 0 24 24' fill='currentColor'>
              <path d='M12 2l2.4 4.9 5.4.8-3.9 3.8.9 5.4L12 18.9 7.2 19.7l.9-5.4L4.2 10.5l5.4-.8L12 2z' />
            </svg>
          </div>
          <div className='leading-tight'>
            <h1 className='text-lg font-bold tracking-tight text-ink'>
              EngineRoom
            </h1>
            <p className='text-[11px] text-ink-faint'>
              Revisão de partidas com Stockfish
            </p>
          </div>
        </div>
        <button
          type='button'
          onClick={() => setSettingsOpen(true)}
          className='rounded-lg border border-edge bg-panel-2/60 p-2 text-ink-dim transition hover:bg-panel-3 hover:text-ink'
          aria-label='Configurações'
          title='Configurações'
        >
          <svg
            width='18'
            height='18'
            viewBox='0 0 24 24'
            fill='none'
            stroke='currentColor'
            strokeWidth='1.8'
            strokeLinecap='round'
            strokeLinejoin='round'
          >
            <circle cx='12' cy='12' r='3' />
            <path d='M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z' />
          </svg>
        </button>
      </header>

      <div
        className={`flex w-full flex-col gap-10 ${hasGames ? 'max-w-5xl lg:flex-row' : 'max-w-xl'}`}
      >
        <div className='w-full max-w-xl shrink-0'>
          <h2 className='mb-1 text-2xl font-bold text-ink'>Revisar partida</h2>
          <p className='mb-6 text-sm text-ink-dim'>
            Importe um PGN e ajuste a qualidade da análise da engine.
          </p>

          <div className='rounded-2xl border border-edge bg-panel/80 p-5 shadow-xl shadow-black/30'>
            <PgnImporter value={pgn} onChange={setPgn} />

            {/* Validation feedback */}
            <div className='mt-3 min-h-[2.25rem]'>
              {pgn.trim().length === 0 ? null : parse.ok ? (
                <div className='flex flex-wrap items-center gap-x-4 gap-y-1 rounded-lg border border-good/30 bg-good/10 px-3 py-2 text-sm'>
                  <span className='font-semibold text-ink'>
                    {parse.meta.white}
                    {parse.meta.whiteElo ? (
                      <span className='ml-1 text-ink-faint'>
                        ({parse.meta.whiteElo})
                      </span>
                    ) : null}
                  </span>
                  <span className='text-ink-faint'>vs</span>
                  <span className='font-semibold text-ink'>
                    {parse.meta.black}
                    {parse.meta.blackElo ? (
                      <span className='ml-1 text-ink-faint'>
                        ({parse.meta.blackElo})
                      </span>
                    ) : null}
                  </span>
                  <span className='text-ink-dim'>·</span>
                  <span className='text-good'>
                    {resultLabel(parse.meta.result)}
                  </span>
                  <span className='text-ink-dim'>·</span>
                  <span className='text-ink-dim'>
                    {Math.ceil(parse.meta.plies / 2)} lances
                  </span>
                </div>
              ) : (
                <div className='flex items-center gap-2 rounded-lg border border-blunder/30 bg-blunder/10 px-3 py-2 text-sm text-blunder'>
                  <svg
                    width='16'
                    height='16'
                    viewBox='0 0 24 24'
                    fill='none'
                    stroke='currentColor'
                    strokeWidth='2'
                    strokeLinecap='round'
                    strokeLinejoin='round'
                  >
                    <circle cx='12' cy='12' r='10' />
                    <line x1='12' y1='8' x2='12' y2='12' />
                    <line x1='12' y1='16' x2='12.01' y2='16' />
                  </svg>
                  {parse.error}
                </div>
              )}
            </div>

            <div className='my-5 h-px bg-edge-soft' />

            <EngineDepthSlider
              value={tierId}
              onChange={(t) => setTierId(t.id)}
              lines={lines}
              onLinesChange={setLines}
            />

            <button
              type='button'
              disabled={!canStart}
              onClick={() =>
                parse.ok &&
                onStart({
                  pgn,
                  meta: parse.meta,
                  engine,
                  lines,
                })
              }
              className={`mt-5 flex w-full items-center justify-center gap-2 rounded-xl px-4 py-3 text-sm font-semibold transition ${
                canStart
                  ? 'bg-brand text-bg hover:bg-brand-strong active:scale-[0.99]'
                  : 'cursor-not-allowed bg-panel-3 text-ink-faint'
              }`}
            >
              Analisar partida
              <svg
                width='16'
                height='16'
                viewBox='0 0 24 24'
                fill='none'
                stroke='currentColor'
                strokeWidth='2.5'
                strokeLinecap='round'
                strokeLinejoin='round'
              >
                <line x1='5' y1='12' x2='19' y2='12' />
                <polyline points='12 5 19 12 12 19' />
              </svg>
            </button>
          </div>

          <p className='mt-4 text-center text-xs text-ink-faint'>
            Toda a análise acontece localmente — seu PGN não sai do seu
            computador.
          </p>
        </div>

        {hasGames && (
          <ReviewedGamesList
            games={games}
            onOpen={openStored}
            onDelete={removeStored}
            onReanalyze={reanalyzeStored}
          />
        )}
      </div>

      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  )
}
