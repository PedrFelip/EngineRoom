import {
  ChartNoAxesCombined,
  ListOrdered,
  ListTree,
  Target,
} from 'lucide-react'
import { memo, type ReactNode, useState } from 'react'
import { useStore } from 'zustand'
import type { ReviewStore } from '../../lib/review-store'
import { useSettings } from '../../lib/settings-context'
import MoveList from '../MoveList'
import ReviewSummary from '../ReviewSummary'
import { SegmentedControl } from '../ui/segmented-control'

interface Props {
  store: ReviewStore
  onNavigate: () => void
  analysisEnabled: boolean
  analysis: ReactNode
  graph: ReactNode
}

export default memo(function ReviewSidebar({
  store,
  onNavigate,
  analysisEnabled,
  analysis,
  graph,
}: Props) {
  const [selectedView, setView] = useState<'moves' | 'analysis' | 'summary'>(
    'moves',
  )
  const [learningColor, setLearningColor] = useState<'all' | 'w' | 'b'>('all')
  const view =
    !analysisEnabled && selectedView === 'analysis' ? 'moves' : selectedView
  const result = useStore(store, (state) => state.result)
  const currentPly = useStore(store, (state) => state.currentPly)
  const variation = useStore(store, (state) => state.variation)
  const variations = useStore(store, (state) => state.variations)
  const showVariationFeedback = useSettings(
    (state) => state.settings.reviewMoveFeedbackEnabled,
  )
  const criticalMoves =
    result?.moves.filter(
      (move) =>
        move.classification === 'erro' || move.classification === 'blunder',
    ) ?? []
  const filteredCriticalMoves = criticalMoves.filter(
    (move) => learningColor === 'all' || move.color === learningColor,
  )
  const tabs = [
    { id: 'moves', label: 'Lances', icon: ListOrdered },
    ...(analysisEnabled
      ? [{ id: 'analysis', label: 'Análise', icon: ListTree } as const]
      : []),
    { id: 'summary', label: 'Resumo', icon: ChartNoAxesCombined },
  ] as const
  return (
    <aside className='flex min-w-0 flex-col gap-3'>
      <SegmentedControl
        label='Painéis da revisão'
        value={view}
        options={tabs}
        onChange={setView}
      />
      <div hidden={view !== 'analysis'}>{analysis}</div>
      {result && (
        <div hidden={view !== 'summary'}>
          <div className='flex flex-col gap-3'>
            {graph}
            <ReviewSummary result={result} />
          </div>
        </div>
      )}
      {result && (
        <div
          hidden={view !== 'moves'}
          className='surface-glass overflow-hidden rounded-2xl border border-edge p-3'
        >
          <div className='flex items-center justify-between px-1 py-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase'>
            <span className='flex items-center gap-1.5'>
              <ListOrdered size={13} strokeWidth={2.2} aria-hidden='true' />
              Lances
            </span>
            <span className='font-mono text-[10px] normal-case'>
              {Math.ceil(result.moves.length / 2)}
            </span>
          </div>
          <p className='mb-3 px-1 text-xs leading-relaxed text-ink-faint'>
            Selecione um lance ou mova as peças para explorar uma alternativa.
          </p>
          <div className='max-h-[50vh] overflow-y-auto pr-1 lg:max-h-[65vh]'>
            <MoveList
              moves={result.moves}
              currentPly={variation ? -1 : currentPly}
              onSelect={(ply) => {
                onNavigate()
                store.goTo(ply)
              }}
              onBranchFrom={(ply) => {
                onNavigate()
                store.goTo(ply)
              }}
              variations={variations}
              activeVariation={variation}
              onSelectVariation={(variationId, path) => {
                onNavigate()
                store.goToVariation(variationId, path)
              }}
              showVariationFeedback={showVariationFeedback}
            />
          </div>
        </div>
      )}
      {view === 'moves' && criticalMoves.length > 0 && (
        <div className='rounded-2xl border border-edge-soft bg-panel-2/50 p-4'>
          <h3 className='flex items-center gap-2 text-xs font-semibold text-ink'>
            <Target size={14} aria-hidden='true' />
            Momentos para aprender
          </h3>
          <p className='mt-1.5 text-xs leading-relaxed text-ink-faint'>
            Volte aos erros e explore o que poderia ter sido jogado.
          </p>
          <SegmentedControl
            className='mt-3'
            label='Filtrar momentos para aprender por cor'
            value={learningColor}
            options={[
              { id: 'all', label: 'Todas' },
              { id: 'w', label: 'Brancas' },
              { id: 'b', label: 'Pretas' },
            ]}
            onChange={setLearningColor}
          />
          {filteredCriticalMoves.length === 0 && (
            <p role='status' className='mt-3 text-xs text-ink-faint'>
              Nenhum erro das {learningColor === 'w' ? 'brancas' : 'pretas'}{' '}
              nesta partida.
            </p>
          )}
          <div className='mt-3 flex max-h-28 flex-wrap gap-1.5 overflow-y-auto'>
            {filteredCriticalMoves.map((move) => (
              <button
                key={move.ply}
                type='button'
                title={`Revisar ${move.san}`}
                onClick={() => {
                  onNavigate()
                  store.goTo(move.ply)
                  setView('moves')
                }}
                className='rounded-lg border border-edge px-2.5 py-1.5 font-mono text-xs text-ink-dim transition hover:border-brand/40 hover:bg-brand/10 hover:text-brand'
              >
                {Math.ceil(move.ply / 2)}
                {move.color === 'w' ? '.' : '…'} {move.san}
              </button>
            ))}
          </div>
        </div>
      )}
    </aside>
  )
})
