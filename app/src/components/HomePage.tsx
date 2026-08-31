import {
  ArrowRight,
  Bot,
  CircleAlert,
  FileUp,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { resolveEngineTier } from '../lib/engine-tier'
import { deleteGame, getGame, listGames, storedToConfig } from '../lib/games'
import { parsePgn, resultLabel } from '../lib/pgn'
import type { EngineMode, GameSummary, ReviewConfig } from '../types'
import EngineTierSelector, { DEFAULT_TIME_MS } from './EngineTierSelector'
import PgnImporter from './PgnImporter'
import ReviewedGamesList from './ReviewedGamesList'
import SettingsModal from './SettingsModal'
import { Button } from './ui/button'

interface Props {
  onStart: (config: ReviewConfig) => void
}

export default function HomePage({ onStart }: Props) {
  const [pgn, setPgn] = useState('')
  const [depth, setDepth] = useState(20)
  const [mode, setMode] = useState<EngineMode>('depth')
  const [movetimeMs, setMovetimeMs] = useState<number>(DEFAULT_TIME_MS)
  const [lines, setLines] = useState(1)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [games, setGames] = useState<GameSummary[]>([])

  const parse = useMemo(() => parsePgn(pgn), [pgn])
  const engine = useMemo(() => resolveEngineTier(depth), [depth])
  const canStart = parse.ok && pgn.trim().length > 0
  const plies = parse.ok ? parse.meta.plies : 0

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
    <div className='flex min-h-full flex-col items-center overflow-x-hidden px-4 py-8 md:px-6 md:py-10 lg:px-8'>
      {/* Brand */}
      <header className='mb-8 flex w-full max-w-xl items-center justify-between md:max-w-6xl'>
        <div className='flex items-center gap-2.5'>
          <div className='brand-mark flex h-9 w-9 items-center justify-center rounded-lg bg-brand text-bg'>
            <Bot size={20} strokeWidth={2} aria-hidden='true' />
          </div>
          <div className='leading-tight'>
            <h1 className='text-lg font-bold tracking-tight text-ink'>
              EngineRoom
            </h1>
            <p className='text-[11px] font-medium tracking-wide text-ink-faint'>
              LOCAL CHESS INTELLIGENCE
            </p>
          </div>
        </div>
        <Button
          onClick={() => setSettingsOpen(true)}
          variant='ghost'
          size='icon'
          className='border-edge bg-panel-2/60'
          aria-label='Configurações'
          title='Configurações'
        >
          <Settings size={18} strokeWidth={1.8} aria-hidden='true' />
        </Button>
      </header>

      <div className='flex w-full max-w-xl flex-col gap-8 md:max-w-6xl md:flex-row md:gap-10'>
        <div className='w-full max-w-xl shrink-0 md:max-w-md lg:max-w-xl'>
          <p className='section-kicker mb-2'>01 · nova análise</p>
          <h2 className='mb-1 text-2xl font-bold tracking-tight text-ink'>
            Revise o que decidiu no tabuleiro.
          </h2>
          <p className='mb-6 text-sm text-ink-dim'>
            Importe um PGN e coloque o Stockfish para encontrar os pontos de
            virada.
          </p>

          <div className='analysis-surface surface-glass elev-card rounded-2xl border border-edge p-5'>
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
                  <CircleAlert
                    size={16}
                    strokeWidth={2}
                    shrink-0
                    aria-hidden='true'
                  />
                  {parse.error}
                </div>
              )}
            </div>

            <div className='my-5 h-px bg-edge-soft' />

            <EngineTierSelector
              mode={mode}
              depth={depth}
              movetimeMs={movetimeMs}
              lines={lines}
              plies={plies}
              onModeChange={setMode}
              onDepthChange={setDepth}
              onMovetimeChange={setMovetimeMs}
              onLinesChange={setLines}
            />

            <Button
              disabled={!canStart}
              onClick={() =>
                parse.ok &&
                onStart({
                  pgn,
                  meta: parse.meta,
                  engine,
                  mode,
                  ...(mode === 'time' ? { movetimeMs } : {}),
                  lines,
                })
              }
              variant={canStart ? 'default' : 'ghost'}
              className={`mt-5 h-10 w-full ${
                canStart
                  ? ''
                  : 'cursor-not-allowed border-edge bg-panel-3 text-ink-faint'
              }`}
            >
              Analisar partida
              <ArrowRight size={16} strokeWidth={2.5} aria-hidden='true' />
            </Button>
          </div>
        </div>

        {hasGames ? (
          <ReviewedGamesList
            games={games}
            onOpen={openStored}
            onDelete={removeStored}
            onReanalyze={reanalyzeStored}
          />
        ) : (
          <aside className='w-full min-w-0 flex-1'>
            <p className='section-kicker mb-2'>Como funciona</p>
            <h2 className='mb-3 text-lg font-semibold tracking-tight text-ink'>
              Da partida ao padrão.
            </h2>
            <div className='analysis-surface surface-glass elev-card rounded-2xl border border-edge p-5'>
              <ol className='flex flex-col gap-4'>
                <li className='flex gap-3'>
                  <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 text-brand'>
                    <FileUp size={14} strokeWidth={2.2} aria-hidden='true' />
                  </span>
                  <div>
                    <p className='text-sm font-semibold text-ink'>
                      Importe um PGN
                    </p>
                    <p className='mt-0.5 text-xs text-ink-dim'>
                      Arraste um arquivo .pgn ou cole a notação diretamente.
                    </p>
                  </div>
                </li>
                <li className='flex gap-3'>
                  <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 text-brand'>
                    <SlidersHorizontal
                      size={14}
                      strokeWidth={2.2}
                      aria-hidden='true'
                    />
                  </span>
                  <div>
                    <p className='text-sm font-semibold text-ink'>
                      Ajuste a engine
                    </p>
                    <p className='mt-0.5 text-xs text-ink-dim'>
                      Escolha profundidade, tempo por lance e linhas candidatas.
                    </p>
                  </div>
                </li>
                <li className='flex gap-3'>
                  <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 text-brand'>
                    <Sparkles size={14} strokeWidth={2.2} aria-hidden='true' />
                  </span>
                  <div>
                    <p className='text-sm font-semibold text-ink'>
                      Receba a revisão
                    </p>
                    <p className='mt-0.5 text-xs text-ink-dim'>
                      Cada lance é classificado com precisão e win% estimado.
                    </p>
                  </div>
                </li>
              </ol>
              <div className='mt-5 flex items-start gap-2 border-t border-edge-soft pt-4 text-xs text-ink-faint'>
                <ShieldCheck
                  size={14}
                  strokeWidth={2}
                  className='mt-0.5 shrink-0'
                  aria-hidden='true'
                />
                <span>
                  Toda a análise acontece localmente — seu PGN não sai do seu
                  computador.
                </span>
              </div>
            </div>
          </aside>
        )}
      </div>

      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onGamesCleared={() => setGames([])}
      />
    </div>
  )
}
