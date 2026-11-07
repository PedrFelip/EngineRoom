import { Chess } from 'chess.js'
import { useMemo } from 'react'
import { useStore } from 'zustand'
import { bMoveLabel } from '../../lib/review-b-moves'
import type { BValue } from '../../lib/review-protocol'
import {
  selectBDecisionPosition,
  selectDisplayedPosition,
} from '../../lib/review-selectors'
import { type UseReview, useReview } from '../../lib/use-review'
import type { ReviewConfig } from '../../types'
import Board from '../Board'
import ReviewLiveStatus from './ReviewLiveStatus'

function valueLabel(value: BValue | null): string {
  if (!value) return 'Indisponível'
  if (value.kind === 'cp')
    return `${value.value > 0 ? '+' : ''}${(value.value / 100).toFixed(2)}`
  if (value.kind === 'mate') return `Mate ${value.moves}`
  return value.outcome
}

export default function BReviewScreen({
  config,
  onExit,
}: {
  config: ReviewConfig
  onExit: () => void
}) {
  const review = useReview(config)
  return <BReviewView config={config} onExit={onExit} review={review} />
}

export function BReviewView({
  config,
  onExit,
  review,
}: {
  config: ReviewConfig
  onExit: () => void
  review: UseReview
}) {
  const moves = useMemo(() => {
    const game = new Chess()
    try {
      game.loadPgn(config.pgn)
      return game.history({ verbose: true }).map((move, index) => ({
        move,
        ply: index + 1,
      }))
    } catch {
      return []
    }
  }, [config.pgn])
  const before = useStore(review.store, selectBDecisionPosition)
  const live = useStore(review.store, selectDisplayedPosition)
  const result = review.bResult
  const position = result?.positions.find(
    (item) => item.ply === review.currentPly,
  )
  const decision = result?.decisions.find(
    (item) => item.ply === review.currentPly,
  )
  return (
    <div className='review-shell mx-auto max-w-6xl space-y-4 p-4'>
      <header className='flex items-center justify-between gap-3'>
        <div>
          <h1 className='text-xl font-semibold'>Experimental B</h1>
          <p className='text-sm text-ink-dim'>
            Experimental · salvamento no histórico sujeito a disponibilidade
          </p>
        </div>
        <button
          type='button'
          className='rounded-lg border border-edge p-2'
          onClick={onExit}
        >
          Voltar
        </button>
      </header>
      {review.status === 'running' && (
        <p role='status'>
          Analisando:{' '}
          {review.bProgress?.stage === 'verification'
            ? 'verificação'
            : 'cobertura'}{' '}
          {review.bProgress
            ? `${review.bProgress.completed} / ${review.bProgress.total}`
            : '…'}
        </p>
      )}
      {review.status === 'error' && (
        <p role='alert'>Falha na análise: {review.error}</p>
      )}
      {result && (
        <div className='grid gap-5 lg:grid-cols-2'>
          <div className='min-w-0 space-y-3'>
            {position && (
              <Board fen={position.fen} orientation={review.orientation} />
            )}
            <ReviewLiveStatus store={review.store} />
            {live && (
              <div>
                <p>
                  Análise ao vivo da posição exibida · avaliação pelas brancas
                </p>
                {live.lines.map((line) => (
                  <p key={line.multipv}>{line.pv.join(' ')}</p>
                ))}
              </div>
            )}
            <div className='flex flex-wrap gap-2'>
              <button type='button' onClick={review.first}>
                Início
              </button>
              <button type='button' onClick={review.prev}>
                Anterior
              </button>
              <button type='button' onClick={review.next}>
                Próximo
              </button>
              <button type='button' onClick={review.last}>
                Fim
              </button>
              <button type='button' onClick={review.flip}>
                Virar
              </button>
            </div>
            <div className='flex flex-wrap gap-2'>
              <button
                type='button'
                onClick={() => review.goTo(0)}
                aria-pressed={review.currentPly === 0}
              >
                Início
              </button>
              {moves.map(({ move, ply }) => (
                <button
                  type='button'
                  key={ply}
                  onClick={() => review.goTo(ply)}
                  aria-pressed={review.currentPly === ply}
                  className='rounded border border-edge px-2 py-1'
                >
                  {bMoveLabel(move)}
                </button>
              ))}
            </div>
          </div>
          <section className='min-w-0 space-y-3 rounded-xl border border-edge p-4'>
            <h2 className='font-semibold'>Posição {review.currentPly}</h2>
            <p>
              Resultado:{' '}
              {review.status === 'running'
                ? 'em andamento'
                : result.status === 'unresolved'
                  ? 'com pendências'
                  : 'protocolo concluído'}
            </p>
            {decision ? (
              <>
                <p>
                  Revisão {decision.version} ·{' '}
                  {decision.status === 'verified'
                    ? 'verificada'
                    : decision.status === 'unresolved'
                      ? 'pendente'
                      : 'preliminar'}
                </p>
                <p>
                  Melhor: {valueLabel(decision.best)} · Jogado:{' '}
                  {valueLabel(decision.played)}
                </p>
                <p>Melhor lance (UCI): {decision.bestUci ?? 'indisponível'}</p>
                {decision.reason && <p>Pendência: {decision.reason}</p>}
                <h3 className='font-semibold'>
                  Linhas antes do lance selecionado
                </h3>
                {before && (
                  <Board fen={before.fen} orientation={review.orientation} />
                )}
                {decision.lines.length ? (
                  decision.lines.map((line) => (
                    <p key={line.pv.join(' ')}>
                      Profundidade {line.depth} · {valueLabel(line.value)} ·{' '}
                      {line.pv.join(' ') || '—'}
                    </p>
                  ))
                ) : (
                  <p>Sem linhas disponíveis.</p>
                )}
              </>
            ) : (
              <p>Não há decisão para esta posição.</p>
            )}
            <p className='text-sm text-ink-dim'>
              Valores relativos ao jogador antes do lance selecionado.
            </p>
          </section>
        </div>
      )}
    </div>
  )
}
