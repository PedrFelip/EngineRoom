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
import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ADAPTIVE_PROFILES,
  adaptiveProfileForKind,
} from '../lib/adaptive-analysis'
import { resolveEngineTier } from '../lib/engine-tier'
import { deleteGame, getGame, listGames, storedToConfig } from '../lib/games'
import { type PgnParseResult, parsePgn, resultLabel } from '../lib/pgn'
import type {
  AnalysisKind,
  EngineMode,
  GameCursor,
  GameSummary,
  ReviewConfig,
} from '../types'
import EngineTierSelector, { DEFAULT_TIME_MS } from './EngineTierSelector'
import PgnImporter from './PgnImporter'
import ReviewedGamesList from './ReviewedGamesList'
import SettingsModal from './SettingsModal'
import { Button } from './ui/button'

interface Props {
  onStart: (config: ReviewConfig) => void
}

interface PgnValidation {
  source: string
  result: PgnParseResult
}

const HISTORY_PAGE_SIZE = 50

function initialPgnValidation(): PgnValidation {
  return { source: '', result: parsePgn('') }
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
  const [gamesTotal, setGamesTotal] = useState(0)
  const [nextGamesCursor, setNextGamesCursor] = useState<GameCursor | null>(
    null,
  )
  const [loadingMoreGames, setLoadingMoreGames] = useState(false)
  const [validation, setValidation] =
    useState<PgnValidation>(initialPgnValidation)

  useEffect(() => {
    if (validation.source === pgn) return
    const timer = window.setTimeout(() => {
      setValidation({ source: pgn, result: parsePgn(pgn) })
    }, 250)
    return () => window.clearTimeout(timer)
  }, [pgn, validation.source])

  const parse = validation.result
  const validating = pgn.trim().length > 0 && validation.source !== pgn
  const engine = useMemo(() => resolveEngineTier(depth), [depth])
  const canStart = !validating && validation.source === pgn && parse.ok
  const plies = canStart && parse.ok ? parse.meta.plies : 0
  const adaptiveProfile = adaptiveProfileForKind(analysisKind)

  useEffect(() => {
    let cancelled = false
    listGames(HISTORY_PAGE_SIZE)
      .then((page) => {
        if (cancelled) return
        setGames(page.games)
        setGamesTotal(page.total)
        setNextGamesCursor(page.nextCursor)
      })
      .catch((e) => console.warn('Falha ao listar partidas analisadas:', e))
    return () => {
      cancelled = true
    }
  }, [])

  const importPgn = useCallback(function importPgn(nextPgn: string): void {
    setPgn(nextPgn)
    setValidation({ source: nextPgn, result: parsePgn(nextPgn) })
  }, [])

  const openStored = useCallback(
    function openStored(id: number): void {
      void getGame(id)
        .then((game) => {
          if (game) onStart(storedToConfig(game))
        })
        .catch((error) => console.warn('Falha ao abrir partida:', error))
    },
    [onStart],
  )

  const removeStored = useCallback(function removeStored(id: number): void {
    void deleteGame(id)
      .then(() => {
        setGames((previous) => previous.filter((game) => game.id !== id))
        setGamesTotal((total) => Math.max(0, total - 1))
      })
      .catch((error) => console.warn('Falha ao excluir partida:', error))
  }, [])

  const reanalyzeStored = useCallback(
    function reanalyzeStored(id: number): void {
      void getGame(id)
        .then((game) => {
          if (!game) return
          importPgn(game.pgn)
          window.scrollTo({ top: 0, behavior: 'smooth' })
        })
        .catch((error) => console.warn('Falha ao carregar partida:', error))
    },
    [importPgn],
  )

  const hasGames = games.length > 0

  const loadMoreGames = useCallback(
    function loadMoreGames(): void {
      if (!nextGamesCursor || loadingMoreGames) return
      setLoadingMoreGames(true)
      void listGames(HISTORY_PAGE_SIZE, nextGamesCursor)
        .then((page) => {
          setGames((previous) => [...previous, ...page.games])
          setGamesTotal(page.total)
          setNextGamesCursor(page.nextCursor)
        })
        .catch((error) =>
          console.warn('Falha ao carregar mais partidas analisadas:', error),
        )
        .finally(() => setLoadingMoreGames(false))
    },
    [loadingMoreGames, nextGamesCursor],
  )

  function startReview(): void {
    if (!canStart || !parse.ok) return
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
            <PgnImporter value={pgn} onChange={setPgn} onImport={importPgn} />

            {/* Validation feedback */}
            <div className='mt-3 min-h-[2.25rem]'>
              {pgn.trim().length === 0 ? null : validating ? (
                <p className='px-3 py-2 text-sm text-ink-dim'>Validando PGN…</p>
              ) : parse.ok ? (
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

            <Button
              disabled={!canStart}
              onClick={startReview}
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
            total={gamesTotal}
            hasMore={nextGamesCursor !== null}
            loadingMore={loadingMoreGames}
            onOpen={openStored}
            onDelete={removeStored}
            onReanalyze={reanalyzeStored}
            onLoadMore={loadMoreGames}
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
