import { useEffect, useMemo, useState } from 'react'
import {
  ADAPTIVE_PROFILES,
  adaptiveProfileForKind,
} from '../lib/adaptive-analysis'
import { resolveEngineTier } from '../lib/engine-tier'
import { deleteGame, getGame, listGames, storedToConfig } from '../lib/games'
import { parsePgn, resultLabel } from '../lib/pgn'
import type {
  AnalysisKind,
  EngineMode,
  GameSummary,
  ReviewConfig,
} from '../types'
import EngineTierSelector, { DEFAULT_TIME_MS } from './EngineTierSelector'
import PgnImporter from './PgnImporter'
import ReviewedGamesList from './ReviewedGamesList'
import SettingsModal from './SettingsModal'

interface Props {
  onStart: (config: ReviewConfig) => void
}

export default function HomePage({ onStart }: Props) {
  const [pgn, setPgn] = useState('')
  const [depth, setDepth] = useState(20)
  const [mode, setMode] = useState<EngineMode>('depth')
  const [movetimeMs, setMovetimeMs] = useState<number>(DEFAULT_TIME_MS)
  const [lines, setLines] = useState(1)
  const [analysisKind, setAnalysisKind] = useState<AnalysisKind>('auto-fast')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [games, setGames] = useState<GameSummary[]>([])

  const parse = useMemo(() => parsePgn(pgn), [pgn])
  const engine = useMemo(() => resolveEngineTier(depth), [depth])
  const canStart = parse.ok && pgn.trim().length > 0
  const plies = parse.ok ? parse.meta.plies : 0
  const adaptiveProfile = adaptiveProfileForKind(analysisKind)

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

  const startReview = () => {
    if (!parse.ok) return
    onStart({
      pgn,
      meta: parse.meta,
      engine,
      mode: adaptiveProfile ? 'time' : mode,
      analysisKind,
      ...(!adaptiveProfile && mode === 'time' ? { movetimeMs } : {}),
      lines: adaptiveProfile?.triageMultipv ?? lines,
    })
  }

  return (
    <div className='flex min-h-full flex-col items-center overflow-x-hidden px-4 py-8 md:px-6 md:py-10 lg:px-8'>
      {/* Brand */}
      <header className='mb-8 flex w-full max-w-xl items-center justify-between md:max-w-6xl'>
        <div className='flex items-center gap-2.5'>
          <div className='flex h-9 w-9 items-center justify-center rounded-lg bg-brand text-bg'>
            <svg
              width='20'
              height='20'
              viewBox='0 0 24 24'
              fill='currentColor'
              aria-hidden='true'
            >
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
            aria-hidden='true'
          >
            <circle cx='12' cy='12' r='3' />
            <path d='M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z' />
          </svg>
        </button>
      </header>

      <div className='flex w-full max-w-xl flex-col gap-8 md:max-w-6xl md:flex-row md:gap-10'>
        <div className='w-full max-w-xl shrink-0 md:max-w-md lg:max-w-xl'>
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
                    aria-hidden='true'
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

            <div className='mb-3 grid grid-cols-2 gap-1 rounded-lg bg-panel-3/60 p-1'>
              <button
                type='button'
                onClick={() =>
                  setAnalysisKind((current) =>
                    current === 'manual' ? 'auto-fast' : current,
                  )
                }
                className={`rounded-md px-3 py-2 text-sm font-semibold transition ${
                  analysisKind !== 'manual'
                    ? 'bg-brand text-bg shadow'
                    : 'text-ink-dim hover:text-ink'
                }`}
              >
                Automático
              </button>
              <button
                type='button'
                onClick={() => setAnalysisKind('manual')}
                className={`rounded-md px-3 py-2 text-sm font-semibold transition ${
                  analysisKind === 'manual'
                    ? 'bg-brand text-bg shadow'
                    : 'text-ink-dim hover:text-ink'
                }`}
              >
                Manual
              </button>
            </div>

            {analysisKind === 'manual' ? (
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
            ) : (
              <div className='rounded-xl border border-edge bg-panel-2/60 p-5'>
                <div className='mb-4'>
                  <h3 className='text-sm font-semibold uppercase tracking-wide text-ink-dim'>
                    Perfil automático
                  </h3>
                  <p className='mt-0.5 text-xs text-ink-faint'>
                    Mapeia alternativas e aprofunda somente posições críticas
                  </p>
                </div>
                <div className='grid grid-cols-2 gap-2'>
                  {(['auto-fast', 'auto-deep'] as const).map((kind) => {
                    const profile =
                      ADAPTIVE_PROFILES[kind === 'auto-fast' ? 'fast' : 'deep']
                    const active = analysisKind === kind
                    return (
                      <button
                        key={kind}
                        type='button'
                        onClick={() => setAnalysisKind(kind)}
                        className={`rounded-lg px-3 py-3 text-left transition ${
                          active
                            ? 'bg-brand/15 ring-1 ring-brand/50'
                            : 'bg-panel-3/40 hover:bg-panel-3'
                        }`}
                      >
                        <span
                          className={`block text-sm font-semibold ${
                            active ? 'text-brand' : 'text-ink'
                          }`}
                        >
                          {profile.id === 'fast' ? 'Rápido' : 'Profundo'}
                        </span>
                        <span className='mt-1 block text-[11px] text-ink-faint'>
                          Triagem MultiPV {profile.triageMultipv} · até{' '}
                          {profile.highMs / 1000}s nos críticos
                        </span>
                      </button>
                    )
                  })}
                </div>
                <p className='mt-3 text-center text-xs text-ink-dim'>
                  A segunda passagem revisita apenas os pares antes/depois dos
                  lances prioritários.
                </p>
              </div>
            )}

            <button
              type='button'
              disabled={!canStart}
              onClick={startReview}
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
                aria-hidden='true'
              >
                <line x1='5' y1='12' x2='19' y2='12' />
                <polyline points='12 5 19 12 12 19' />
              </svg>
            </button>
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
            <h2 className='mb-3 text-sm font-semibold tracking-wide text-ink-dim uppercase'>
              Como funciona
            </h2>
            <div className='rounded-2xl border border-edge bg-panel/80 p-5 shadow-xl shadow-black/30'>
              <ol className='flex flex-col gap-4'>
                <li className='flex gap-3'>
                  <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 font-mono text-sm font-bold text-brand'>
                    1
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
                  <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 font-mono text-sm font-bold text-brand'>
                    2
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
                  <span className='flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-brand/15 font-mono text-sm font-bold text-brand'>
                    3
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
                <svg
                  width='14'
                  height='14'
                  viewBox='0 0 24 24'
                  fill='none'
                  stroke='currentColor'
                  strokeWidth='2'
                  strokeLinecap='round'
                  strokeLinejoin='round'
                  className='mt-0.5 shrink-0'
                  aria-hidden='true'
                >
                  <path d='M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z' />
                </svg>
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
