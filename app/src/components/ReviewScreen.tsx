import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import type { RawPosition } from '../lib/analyze'
import { formatEngineTag } from '../lib/engine-tag'
import { evalLabel, sideToMoveAtPly } from '../lib/eval-label'
import { resultLabel } from '../lib/pgn'
import { cpToWinPct } from '../lib/scoring'
import { useSettings } from '../lib/settings-context'
import { playMoveSound } from '../lib/sound'
import { useReview } from '../lib/use-review'
import type { PositionAnalysis, PvLine, ReviewConfig } from '../types'
import Board from './Board'
import CandidateLines from './CandidateLines'
import EvalBar from './EvalBar'
import EvalGraph from './EvalGraph'
import MoveList from './MoveList'
import ReviewSettingsPanel from './ReviewSettingsPanel'
import ReviewSummary from './ReviewSummary'

interface ReviewScreenProps {
  config: ReviewConfig
  onExit: () => void
}

function uciToSquares(uci: string): [string, string] | null {
  if (uci.length < 4) return null
  return [uci.slice(0, 2), uci.slice(2, 4)]
}

/**
 * Converte um RawPosition ao vivo (POV do lado a jogar, gerado pela engine de
 * refino) num PositionAnalysis normalizado para o POV das brancas — preserva
 * ply/fen da posição base e descarta o cp/lines/pv antigo.
 *
 * Quando o refino ainda não preencheu todos os slots multipv que a posição
 * base tinha (vindas do analyzeGame com o SF18), completamos com os slots do
 * `basePosition` — assim o painel nunca regredir pra menos linhas do que já
 * tinha antes do refino começar.
 *
 * Não computa SAN nem classification (esses dependem do fluxo do buildReview);
 * a UI lida com san=null mostrando "—".
 */
function mergeLiveIntoPosition(
  live: RawPosition,
  base: PositionAnalysis,
  stm: 'w' | 'b',
): PositionAnalysis {
  const winPct = stm === 'w' ? cpToWinPct(live.cp) : 100 - cpToWinPct(live.cp)
  const liveLines: PvLine[] = (live.lines ?? []).map((l) => {
    const cp = stm === 'w' ? l.cp : -l.cp
    return {
      multipv: l.multipv,
      san: null,
      cp,
      winPct: stm === 'w' ? cpToWinPct(l.cp) : 100 - cpToWinPct(l.cp),
      pv: l.pv,
    }
  })

  // Completa com slots do base que a live ainda não cobriu (mínimo garantido).
  const liveIndices = new Set(liveLines.map((l) => l.multipv))
  const baseExtras = base.lines.filter((l) => !liveIndices.has(l.multipv))
  const allLines = [...liveLines, ...baseExtras].sort(
    (a, b) => a.multipv - b.multipv,
  )

  return {
    ...base,
    depth: live.depth,
    cp: stm === 'w' ? live.cp : -live.cp,
    winPct,
    pv: live.pv.length > 0 ? live.pv : base.pv,
    lines: allLines.length > 0 ? allLines : base.lines,
  }
}

export default function ReviewScreen({ config, onExit }: ReviewScreenProps) {
  const review = useReview(config)
  const { settings } = useSettings()
  const {
    result,
    status,
    error,
    currentPly,
    orientation,
    livePosition,
    liveWideAvailable,
    liveWideOn,
    setLiveWideOn,
    presets,
    applyPreset,
  } = review

  const basePosition = result?.positions[currentPly] ?? null
  const stm = result ? sideToMoveAtPly(result.moves, currentPly) : 'w'

  // Merge: quando há refinamento ao vivo, sobreponho cp/winPct/lines/pv/depth.
  const position =
    basePosition && livePosition
      ? mergeLiveIntoPosition(livePosition, basePosition, stm)
      : basePosition

  const currentMove =
    currentPly > 0 ? (result?.moves[currentPly - 1] ?? null) : null
  const lastMoveUci = currentMove?.uci ?? null
  const opening = result?.moves.find((m) => m.eco)?.eco ?? null

  const evalBarLabel =
    position && result ? evalLabel(position.cp, position.fen, stm) : undefined

  const [selectedMultipv, setSelectedMultipv] = useState(1)
  // biome-ignore lint/correctness/useExhaustiveDependencies: reseta a linha selecionada sempre que o usuário navega para outro lance
  useEffect(() => {
    setSelectedMultipv(1)
  }, [currentPly])

  // Toca o som do lance apenas ao avançar (currentPly aumenta). Voltar/início
  // não dispara som. Ref rastreia o ply anterior sem causar re-render.
  const prevPlyRef = useRef(currentPly)
  // biome-ignore lint/correctness/useExhaustiveDependencies: dispara só em mudança de ply; settings/result lidos via closure no último render
  useEffect(() => {
    const movedForward = currentPly > prevPlyRef.current
    prevPlyRef.current = currentPly
    if (!movedForward || !settings.soundEnabled) return
    const san = result?.moves[currentPly - 1]?.san
    if (san) playMoveSound(san, settings.soundVolume)
  }, [currentPly])
  const selectedLine =
    position?.lines.find((l) => l.multipv === selectedMultipv) ??
    position?.lines[0]

  const bestArrow = useMemo(() => {
    const uci = selectedLine?.pv[0]
    if (!uci) return null
    const sq = uciToSquares(uci)
    return sq ? { from: sq[0], to: sq[1], brush: 'blue' as const } : null
  }, [selectedLine])

  useEffect(() => {
    if (!result) return
    const onKey = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowLeft':
          review.prev()
          break
        case 'ArrowRight':
          review.next()
          break
        case 'Home':
          review.first()
          break
        case 'End':
          review.last()
          break
        default:
          return
      }
      e.preventDefault()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [result, review.prev, review.next, review.first, review.last])

  return (
    <div className='mx-auto flex min-h-full max-w-6xl flex-col gap-4 px-4 py-6'>
      <header className='flex items-center justify-between gap-4'>
        <div className='min-w-0'>
          <h1 className='truncate text-lg font-bold text-ink'>
            {config.meta.white} <span className='text-ink-faint'>vs</span>{' '}
            {config.meta.black}
          </h1>
          <p className='truncate text-sm text-ink-dim'>
            {resultLabel(config.meta.result)} ·{' '}
            {Math.ceil(config.meta.plies / 2)} lances ·{' '}
            {formatEngineTag({
              mode: config.mode,
              depth:
                config.mode === 'time'
                  ? (config.movetimeMs ?? 0)
                  : config.engine.depth,
              engineTier: config.engine.id,
            })}
            {opening ? ` · ${opening.code} ${opening.name}` : ''}
            {livePosition ? ` · AO VIVO d${livePosition.depth}` : ''}
          </p>
        </div>
        <div className='flex items-center gap-2'>
          <ReviewSettingsPanel
            liveWideAvailable={liveWideAvailable}
            liveWideOn={liveWideOn}
            onToggleWide={setLiveWideOn}
            presets={presets}
            onApplyPreset={applyPreset}
          />
          <button
            type='button'
            onClick={onExit}
            className='rounded-xl bg-panel-3 px-4 py-2 text-sm font-medium text-ink transition hover:bg-edge'
          >
            ← Nova partida
          </button>
        </div>
      </header>

      {status === 'error' && (
        <div className='rounded-xl border border-blunder/50 bg-blunder/10 p-4 text-sm text-blunder'>
          Falha na análise: {error}
        </div>
      )}

      <div className='grid flex-1 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px]'>
        <div className='flex flex-col gap-3'>
          <div className='flex flex-col gap-1'>
            <PlayerTag
              name={
                orientation === 'white' ? config.meta.black : config.meta.white
              }
              elo={
                orientation === 'white'
                  ? config.meta.blackElo
                  : config.meta.whiteElo
              }
              color={orientation === 'white' ? 'b' : 'w'}
            />
            <div className='flex items-stretch gap-2'>
              <EvalBar
                winPct={position?.winPct ?? 50}
                orientation={orientation}
                label={evalBarLabel}
              />
              <div className='min-w-0 flex-1'>
                {position ? (
                  <Board
                    fen={position.fen}
                    orientation={orientation}
                    lastMove={lastMoveUci ? uciToSquares(lastMoveUci) : null}
                    arrows={bestArrow ? [bestArrow] : []}
                    lastMoveClassification={currentMove?.classification ?? null}
                  />
                ) : (
                  <div className='flex aspect-square w-full items-center justify-center rounded-lg border border-edge bg-panel-2/60 text-ink-dim'>
                    {status === 'running' ? 'Analisando…' : '—'}
                  </div>
                )}
              </div>
            </div>
            <PlayerTag
              name={
                orientation === 'white' ? config.meta.white : config.meta.black
              }
              elo={
                orientation === 'white'
                  ? config.meta.whiteElo
                  : config.meta.blackElo
              }
              color={orientation === 'white' ? 'w' : 'b'}
            />
          </div>

          {position?.lines?.length ? (
            <CandidateLines
              lines={position.lines}
              selectedMultipv={selectedMultipv}
              onSelect={setSelectedMultipv}
            />
          ) : null}

          <div className='flex items-center justify-center gap-2 rounded-xl border border-edge bg-panel-2/60 p-2'>
            <NavBtn onClick={review.first} disabled={!result}>
              ⏮
            </NavBtn>
            <NavBtn
              onClick={review.prev}
              disabled={!result || currentPly === 0}
            >
              ‹
            </NavBtn>
            <NavBtn
              onClick={review.next}
              disabled={!result || currentPly >= (result?.moves.length ?? 0)}
            >
              ›
            </NavBtn>
            <NavBtn
              onClick={review.last}
              disabled={!result || currentPly >= (result?.moves.length ?? 0)}
            >
              ⏭
            </NavBtn>
            <NavBtn onClick={review.flip}>⇅</NavBtn>
          </div>
        </div>

        <aside className='flex flex-col gap-4'>
          {result && <ReviewSummary result={result} />}
          {result && (
            <div className='max-h-[50vh] overflow-y-auto rounded-xl border border-edge bg-panel-2/60 p-3'>
              <MoveList
                moves={result.moves}
                currentPly={currentPly}
                onSelect={review.goTo}
              />
            </div>
          )}
        </aside>
      </div>

      {result && (
        <div className='rounded-xl border border-edge bg-panel-2/60 p-3'>
          <div className='mb-1.5 flex items-center justify-between px-1'>
            <span className='text-[11px] font-semibold uppercase tracking-wide text-ink-faint'>
              Avaliação
            </span>
            <span className='text-[11px] text-ink-faint'>
              clique para pular até o lance
            </span>
          </div>
          <EvalGraph
            winPcts={result.positions.map((p) => p.winPct)}
            currentPly={currentPly}
            onSelect={review.goTo}
          />
        </div>
      )}
    </div>
  )
}

function NavBtn({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void
  disabled?: boolean
  children: ReactNode
}) {
  return (
    <button
      type='button'
      onClick={onClick}
      disabled={disabled}
      className='rounded-lg px-3 py-1.5 text-lg text-ink-dim transition hover:bg-panel-3/60 disabled:opacity-30 disabled:hover:bg-transparent'
    >
      {children}
    </button>
  )
}

function PlayerTag({
  name,
  elo,
  color,
}: {
  name: string
  elo: string | null
  color: 'w' | 'b'
}) {
  return (
    <div className='flex items-center gap-2 px-1 text-sm'>
      <span
        className={`inline-block h-3 w-3 shrink-0 rounded-full border border-edge ${
          color === 'w' ? 'bg-white' : 'bg-[#1a1916]'
        }`}
      />
      <span className='font-medium text-ink'>{name}</span>
      {elo ? (
        <span className='font-mono text-xs text-ink-dim'>({elo})</span>
      ) : null}
    </div>
  )
}
