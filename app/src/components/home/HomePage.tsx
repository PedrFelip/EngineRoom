import {
  Archive,
  ArrowLeft,
  ArrowRight,
  Check,
  CircleAlert,
  ShieldCheck,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { adaptiveProfileForKind } from '../../lib/adaptive-analysis'
import { resolveEngineTier } from '../../lib/engine-tier'
import { type PgnParseResult, parsePgn, resultLabel } from '../../lib/pgn'
import type { AnalysisKind, EngineMode, ReviewConfig } from '../../types'
import EngineTierSelector, { DEFAULT_TIME_MS } from '../EngineTierSelector'
import PgnImporter from '../PgnImporter'
import ReviewedGamesList from '../ReviewedGamesList'
import SettingsModal from '../settings/SettingsModal'
import { Button } from '../ui/button'
import { SegmentedControl } from '../ui/segmented-control'
import HomeHeader from './HomeHeader'
import HowItWorks from './HowItWorks'
import { useHistoryController } from './use-history-controller'

interface Props {
  active?: boolean
  onStart: (config: ReviewConfig) => void
}

interface PgnValidation {
  source: string
  result: PgnParseResult
}

function initialPgnValidation(): PgnValidation {
  return { source: '', result: parsePgn('') }
}

export default function HomePage({ onStart, active = true }: Props) {
  const [view, setView] = useState<'new' | 'history'>('new')
  const [step, setStep] = useState<'import' | 'configure'>('import')
  const [readingPgn, setReadingPgn] = useState(false)
  const pageTitle = useRef<HTMLHeadingElement>(null)
  const stepTitle = useRef<HTMLHeadingElement>(null)
  const previousLocation = useRef({ view, step, active })

  useEffect(() => {
    const previous = previousLocation.current
    previousLocation.current = { view, step, active }
    if (!active) return
    if (!previous.active) {
      if (view === 'new' && step === 'configure') stepTitle.current?.focus()
      else pageTitle.current?.focus()
    } else if (view === 'new' && previous.step !== step)
      stepTitle.current?.focus()
    else if (previous.view !== view) pageTitle.current?.focus()
  }, [active, view, step])
  const [pgn, setPgn] = useState('')
  const [depth, setDepth] = useState(20)
  const [mode, setMode] = useState<EngineMode>('depth')
  const [movetimeMs, setMovetimeMs] = useState<number>(DEFAULT_TIME_MS)
  const [lines, setLines] = useState(1)
  const [analysisKind, setAnalysisKind] = useState<AnalysisKind>('auto-fast')
  const [settingsOpen, setSettingsOpen] = useState(false)
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
  const canStart =
    !readingPgn && !validating && validation.source === pgn && parse.ok
  const plies = canStart && parse.ok ? parse.meta.plies : 0
  const adaptiveProfile = adaptiveProfileForKind(analysisKind)

  useEffect(() => {
    if (!active) setSettingsOpen(false)
  }, [active])

  const importPgn = useCallback(function importPgn(nextPgn: string): void {
    setPgn(nextPgn)
    setValidation({ source: nextPgn, result: parsePgn(nextPgn) })
  }, [])

  const {
    games,
    gamesTotal,
    nextGamesCursor,
    loadingHistory,
    loadingMoreGames,
    pendingGame,
    historyError,
    clearGames,
    setHistoryError,
    loadHistory,
    openStored,
    removeStored,
    reanalyzeStored,
    loadMoreGames,
  } = useHistoryController({ active, onStart, importPgn, setView, setStep })
  const hasGames = games.length > 0
  const configuring = view === 'new' && step === 'configure'
  let pageHeading =
    view === 'new'
      ? 'Cada partida, uma descoberta.'
      : 'Seu jogo, em perspectiva.'
  if (configuring) pageHeading = 'Prepare sua análise'

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
    <div className='home-screen flex min-h-screen flex-col items-center px-3 py-3 sm:px-5 sm:py-4 md:px-10 md:py-5'>
      <HomeHeader
        view={view}
        onViewChange={(next) => {
          setHistoryError(null)
          setView(next)
          window.scrollTo({ top: 0 })
        }}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      {view === 'history' && historyError && (
        <p
          role='alert'
          className='mt-5 w-full max-w-6xl rounded-xl border border-blunder/25 bg-blunder/5 p-4 text-sm text-ink'
        >
          {historyError}{' '}
          <button
            type='button'
            onClick={loadHistory}
            disabled={pendingGame !== null}
            className='underline'
          >
            Tentar novamente
          </button>
        </p>
      )}

      <div
        className={`w-full max-w-6xl ${
          configuring ? 'mb-3 pt-4' : 'mb-5 pt-5 sm:mb-6 sm:pt-6'
        }`}
      >
        {!configuring && (
          <p className='mb-1.5 text-xs font-medium tracking-wide text-ink-dim'>
            {view === 'new' ? 'SEU ESPAÇO DE ANÁLISE' : 'SUA BIBLIOTECA'}
          </p>
        )}
        <h2
          id='home-page-title'
          ref={pageTitle}
          tabIndex={-1}
          className={`navigation-focus-target scroll-mt-28 text-2xl font-semibold tracking-tight text-ink ${
            configuring ? '' : 'sm:text-3xl md:text-4xl'
          }`}
        >
          {pageHeading}
        </h2>
        {!configuring && (
          <p className='mt-2 max-w-2xl text-sm leading-relaxed text-ink-dim'>
            {view === 'new'
              ? 'Importe, escolha o ritmo e explore os momentos que decidiram o jogo.'
              : 'Retome suas revisões ou analise uma partida com novas configurações.'}
          </p>
        )}
      </div>

      {view === 'new' ? (
        <div
          className={`grid w-full max-w-6xl items-start gap-5 lg:gap-7 ${
            step === 'import'
              ? 'lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]'
              : ''
          }`}
        >
          <section
            id='new-review'
            aria-labelledby='new-review-title'
            className='min-w-0'
          >
            <div className='home-surface p-3.5 sm:p-5'>
              <div className='mb-3'>
                <h3
                  id='new-review-title'
                  ref={stepTitle}
                  tabIndex={-1}
                  className='navigation-focus-target scroll-mt-28 text-base font-semibold text-ink'
                >
                  {step === 'import'
                    ? 'Importe sua partida'
                    : 'Escolha seu ritmo'}
                </h3>
                <p className='mt-1 text-xs text-ink-dim'>
                  {step === 'import'
                    ? 'Etapa 1 de 2 · Arquivo, PGN ou Chess.com'
                    : 'Etapa 2 de 2 · Escolha como quer revisar'}
                </p>
              </div>
              <ol
                aria-label='Etapas da nova análise'
                className='new-review-steps'
                data-step={step}
              >
                {(['import', 'configure'] as const).map((item, index) => {
                  let state = 'upcoming'
                  if (step === item) state = 'current'
                  else if (index === 0) state = 'complete'
                  return (
                    <li
                      key={item}
                      aria-current={step === item ? 'step' : undefined}
                      className='new-review-step'
                      data-state={state}
                    >
                      <span className='new-review-step-marker'>
                        {step === 'configure' && index === 0 ? (
                          <Check size={12} aria-hidden='true' />
                        ) : (
                          index + 1
                        )}
                      </span>
                      {index === 0 ? 'Partida' : 'Análise'}
                      {step === 'configure' && index === 0 && (
                        <span className='sr-only'> — concluída</span>
                      )}
                    </li>
                  )
                })}
              </ol>
              <div hidden={step !== 'import'}>
                <PgnImporter
                  value={pgn}
                  onChange={setPgn}
                  onImport={importPgn}
                  onReadingChange={setReadingPgn}
                  active={active && view === 'new' && step === 'import'}
                  valid={validation.source === pgn && parse.ok}
                />
              </div>
              {step !== 'import' && (
                <Button
                  variant='ghost'
                  size='sm'
                  onClick={() => setStep('import')}
                >
                  <ArrowLeft size={14} aria-hidden='true' /> Alterar partida
                </Button>
              )}

              {/* Validation feedback */}
              <div
                aria-live='polite'
                aria-atomic='true'
                className={pgn.trim() ? 'mt-3' : undefined}
              >
                {pgn.trim().length === 0 ? null : validating ? (
                  <p className='px-3 py-2 text-sm text-ink-dim'>
                    Validando PGN…
                  </p>
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

              {step === 'configure' && (
                <div>
                  <div className='my-3 h-px bg-edge-soft' />

                  <SegmentedControl
                    className='mb-3'
                    label='Modo de análise'
                    value={analysisKind === 'manual' ? 'manual' : 'auto'}
                    options={[
                      { id: 'auto', label: 'Automático' },
                      { id: 'manual', label: 'Manual' },
                    ]}
                    onChange={(value) =>
                      setAnalysisKind((current) => {
                        if (value === 'manual') return 'manual'
                        return current === 'manual' ? 'auto-fast' : current
                      })
                    }
                  />

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
                    <div className='rounded-xl bg-panel-2/50 p-3'>
                      <div className='mb-3'>
                        <h3 className='text-sm font-medium text-ink'>
                          Perfil automático
                        </h3>
                        <p className='mt-0.5 text-xs text-ink-faint'>
                          Mais atenção aos momentos que decidem a partida.
                        </p>
                      </div>
                      <div className='grid grid-cols-2 gap-2'>
                        {(['auto-fast', 'auto-deep'] as const).map((kind) => {
                          const isFast = kind === 'auto-fast'
                          const active = analysisKind === kind
                          return (
                            <button
                              key={kind}
                              type='button'
                              aria-pressed={active}
                              onClick={() => setAnalysisKind(kind)}
                              className={`rounded-lg px-3 py-2.5 text-left transition ${
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
                                {isFast ? 'Rápido' : 'Profundo'}
                              </span>
                              <span className='mt-1 block text-[11px] text-ink-faint'>
                                {isFast
                                  ? 'Uma visão geral, em menos tempo'
                                  : 'Mais detalhe em posições críticas'}
                              </span>
                            </button>
                          )
                        })}
                      </div>
                      <p className='mt-3 text-center text-xs text-ink-dim'>
                        A análise se adapta à complexidade da sua partida.
                      </p>
                    </div>
                  )}
                </div>
              )}
              <Button
                disabled={!canStart}
                onClick={() =>
                  step === 'import' ? setStep('configure') : startReview()
                }
                variant={canStart ? 'default' : 'ghost'}
                className={`mt-4 h-11 w-full ${step === 'configure' ? 'sticky bottom-3 z-20 shadow-lg' : ''} ${
                  canStart
                    ? ''
                    : 'cursor-not-allowed border-edge bg-panel-3 text-ink-faint'
                }`}
              >
                {step === 'import' ? 'Continuar' : 'Analisar partida'}
                <ArrowRight size={16} strokeWidth={2.5} aria-hidden='true' />
              </Button>
              <p className='mt-3 flex items-center justify-center gap-1.5 text-xs text-ink-dim'>
                <ShieldCheck size={13} aria-hidden='true' /> Stockfish 19 ·
                Análise local e privada
              </p>
            </div>
          </section>

          {step === 'import' && <HowItWorks />}
        </div>
      ) : (
        <div className='w-full max-w-6xl'>
          {loadingHistory ? (
            <div
              role='status'
              className='home-surface p-10 text-center text-sm text-ink-dim'
            >
              Carregando suas partidas…
            </div>
          ) : hasGames || nextGamesCursor !== null ? (
            <>
              {pendingGame !== null && (
                <p role='status' className='mb-3 text-sm text-ink-dim'>
                  Preparando sua partida…
                </p>
              )}
              <ReviewedGamesList
                busy={pendingGame !== null}
                games={games}
                total={gamesTotal}
                hasMore={nextGamesCursor !== null}
                loadingMore={loadingMoreGames}
                onOpen={openStored}
                onDelete={removeStored}
                onReanalyze={reanalyzeStored}
                onLoadMore={loadMoreGames}
              />
            </>
          ) : (
            <section className='home-surface flex flex-col items-center px-6 py-14 text-center'>
              <Archive
                size={28}
                strokeWidth={1.4}
                className='mb-5 text-ink-dim'
                aria-hidden='true'
              />
              <h3 className='text-lg font-semibold text-ink'>
                {historyError
                  ? 'Histórico indisponível'
                  : 'Sua primeira descoberta espera por você.'}
              </h3>
              <p className='mt-2 max-w-sm text-sm leading-relaxed text-ink-dim'>
                {historyError
                  ? 'Tente carregar suas revisões novamente.'
                  : 'As partidas revisadas ficam salvas neste computador para você voltar quando quiser.'}
              </p>
              <Button className='mt-6' onClick={() => setView('new')}>
                Nova análise <ArrowRight size={15} aria-hidden='true' />
              </Button>
            </section>
          )}
        </div>
      )}

      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onGamesCleared={clearGames}
      />
    </div>
  )
}
