import { useStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { selectDisplayedFen } from '../../lib/review-selectors'
import type { ReviewStore } from '../../lib/review-store'
import { useSettings } from '../../lib/settings-context'

export default function ReviewLiveStatus({ store }: { store: ReviewStore }) {
  const enabled = useSettings((state) => state.settings.reviewEngineEnabled)
  const { active, status, error } = useStore(
    store,
    useShallow((state) => ({
      active: state.liveAnalysis.fen === selectDisplayedFen(state),
      status: state.liveAnalysis.status,
      error: state.liveAnalysis.error,
    })),
  )
  if (!enabled || !active || status === 'idle' || status === 'cancelled')
    return null
  return (
    <p className='px-1 text-xs text-ink-faint' role='status'>
      {status === 'running'
        ? 'Analisando posição atual…'
        : `Análise ao vivo indisponível: ${error}`}
    </p>
  )
}
