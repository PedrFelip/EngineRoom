import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import { formatEngineTag } from '../lib/engine-tag'
import { evalLabel, sideToMoveAtPly } from '../lib/eval-label'
import { resultLabel } from '../lib/pgn'
import { cpToWinPct } from '../lib/scoring'
import { useSettings } from '../lib/settings-context'
import { playMoveSound } from '../lib/sound'
import { useReview } from '../lib/use-review'
import type {
  Classification,
  PositionAnalysis,
  PvLine,
  ReviewConfig,
  VariationMove,
} from '../types'
import Board from './Board'
import CandidateLines from './CandidateLines'
import EvalBar from './EvalBar'
import EvalGraph from './EvalGraph'
import MoveList from './MoveList'
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
 * Constrói um PositionAnalysis de exibição a partir de um lance de variação
 * (sem posição-base da linha principal). Reusa os campos que o refino
 * preencheu no lance: afterCp (POV do lado a jogar) e lines (POV das brancas).
 * Retorna null enquanto o lance está pendente (sem eval ainda).
 */
function variationToPosition(
  move: VariationMove,
  stm: 'w' | 'b',
): PositionAnalysis | null {
  if (move.afterCp === undefined && !(move.lines?.length ?? 0)) return null
  const lines = move.lines ?? []
  const primary = lines[0]
  const winPct =
    primary?.winPct ??
    (move.afterCp !== undefined
      ? stm === 'w'
        ? cpToWinPct(move.afterCp)
        : 100 - cpToWinPct(move.afterCp)
      : 50)
  const fallbackLine: PvLine = {
    multipv: 1,
    san: null,
    cp: move.afterCp ?? 0,
    winPct,
    pv: move.bestUci ? [move.bestUci] : [],
  }
  return {
    ply: 0,
    fen: move.fenAfter,
    depth: move.depth ?? 0,
    cp: move.afterCp ?? 0,
    winPct,
    pv: primary?.pv ?? fallbackLine.pv,
    lines: lines.length > 0 ? lines : [fallbackLine],
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
    variations,
    currentVariation,
    currentVariationMove,
    dests,
    turnColor,
    makeMove,
    goToVariation,
    exitVariation,
  } = review

  const basePosition = result?.positions[currentPly] ?? null
  const inVariation = !!currentVariationMove

  // Posição exibida: variação tem sua própria posição sintetizada; linha
  // principal mostra só o resultado do analyzeGame (sem refino ao vivo).
  let position: PositionAnalysis | null
  let stm: 'w' | 'b'
  let lastMoveUci: string | null
  let lastMoveClassification: Classification | null

  if (currentVariationMove) {
    stm = currentVariationMove.color === 'w' ? 'b' : 'w'
    position = variationToPosition(currentVariationMove, stm)
    lastMoveUci = currentVariationMove.uci
    lastMoveClassification = currentVariationMove.classification ?? null
  } else {
    stm = result ? sideToMoveAtPly(result.moves, currentPly) : 'w'
    position = basePosition
    const currentMove =
      currentPly > 0 ? (result?.moves[currentPly - 1] ?? null) : null
    lastMoveUci = currentMove?.uci ?? null
    lastMoveClassification = currentMove?.classification ?? null
  }

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
            {inVariation && currentVariationMove?.depth
              ? ` · VARIAÇÃO d${currentVariationMove.depth}`
              : ''}
          </p>
        </div>
        <button
          type='button'
          onClick={onExit}
          className='rounded-xl bg-panel-3 px-4 py-2 text-sm font-medium text-ink transition hover:bg-edge'
        >
          ← Nova partida
        </button>
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
                    lastMoveClassification={lastMoveClassification}
                    interactive={status === 'done'}
                    turnColor={turnColor}
                    dests={dests}
                    onUserMove={makeMove}
                  />
                ) : (
                  <div className='flex aspect-square w-full items-center justify-center rounded-lg border border-edge bg-panel-2/60 text-ink-dim'>
                    {status === 'running'
                      ? 'Analisando…'
                      : inVariation
                        ? 'Analisando jogada…'
                        : '—'}
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
              disabled={!result || (currentPly === 0 && !inVariation)}
            >
              ‹
            </NavBtn>
            <NavBtn
              onClick={review.next}
              disabled={
                !result ||
                (currentPly >= (result?.moves.length ?? 0) && !inVariation)
              }
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
            {inVariation ? (
              <NavBtn onClick={exitVariation}>✕ variação</NavBtn>
            ) : null}
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
                variations={variations}
                currentVariation={currentVariation}
                onSelectVariation={goToVariation}
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
