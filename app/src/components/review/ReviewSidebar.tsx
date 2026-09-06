import { ListOrdered } from 'lucide-react'
import { memo } from 'react'
import { useStore } from 'zustand'
import type { ReviewStore } from '../../lib/review-store'
import { useSettings } from '../../lib/settings-context'
import MoveList from '../MoveList'
import ReviewSummary from '../ReviewSummary'

interface Props {
  store: ReviewStore
  onNavigate: () => void
}

export default memo(function ReviewSidebar({ store, onNavigate }: Props) {
  const result = useStore(store, (state) => state.result)
  const currentPly = useStore(store, (state) => state.currentPly)
  const variation = useStore(store, (state) => state.variation)
  const variations = useStore(store, (state) => state.variations)
  const showVariationFeedback = useSettings(
    (state) => state.settings.reviewMoveFeedbackEnabled,
  )
  return (
    <aside className='flex flex-col gap-4'>
      {result && <ReviewSummary result={result} />}
      {result && (
        <div className='rounded-xl border border-border bg-card/60 p-2 shadow-sm'>
          <div className='flex items-center justify-between px-1 py-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground uppercase'>
            <span className='flex items-center gap-1.5'>
              <ListOrdered size={13} strokeWidth={2.2} aria-hidden='true' />
              Lances
            </span>
            <span className='font-mono text-[10px] normal-case'>
              {Math.ceil(result.moves.length / 2)}
            </span>
          </div>
          <div className='max-h-[42vh] overflow-y-auto pr-1 lg:max-h-[50vh]'>
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
    </aside>
  )
})
