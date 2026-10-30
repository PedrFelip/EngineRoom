import { useCallback, useEffect, useRef, useState } from 'react'
import { selectActiveVariation } from '../../lib/review-store'
import type { useReview } from '../../lib/use-review'

const EXPLORE_STEP_MS = 650

export function useLinePlayback(review: ReturnType<typeof useReview>) {
  const [isExploringLine, setIsExploringLine] = useState(false)
  const exploreTimerRef = useRef<number | null>(null)

  const stopExplorePlayback = useCallback(
    (endPlaybackAnalysis = true) => {
      if (exploreTimerRef.current !== null) {
        window.clearTimeout(exploreTimerRef.current)
        exploreTimerRef.current = null
      }
      setIsExploringLine(false)
      if (endPlaybackAnalysis) review.endPlaybackAnalysis()
    },
    [review.endPlaybackAnalysis],
  )

  const exploreLineSlowly = useCallback(
    (pv: string[]) => {
      stopExplorePlayback(false)
      review.startPlaybackAnalysis()
      setIsExploringLine(true)
      const path = review.exploreLine(pv)
      const variation = selectActiveVariation(review.store.getState())
      if (!variation || path.length === 0) {
        stopExplorePlayback()
        return
      }
      let cursor = variation.path.length
      let remainingMoves = path.length - cursor
      const advance = () => {
        review.goToVariation(variation.id, path.slice(0, ++cursor))
        remainingMoves--
        if (remainingMoves === 0) {
          exploreTimerRef.current = null
          setIsExploringLine(false)
          review.endPlaybackAnalysis()
          return
        }
        exploreTimerRef.current = window.setTimeout(advance, EXPLORE_STEP_MS)
      }
      if (remainingMoves > 0) {
        exploreTimerRef.current = window.setTimeout(advance, EXPLORE_STEP_MS)
      } else {
        setIsExploringLine(false)
        review.endPlaybackAnalysis()
      }
    },
    [
      review.exploreLine,
      review.store,
      review.goToVariation,
      review.startPlaybackAnalysis,
      review.endPlaybackAnalysis,
      stopExplorePlayback,
    ],
  )

  useEffect(
    () => () => {
      if (exploreTimerRef.current !== null) {
        window.clearTimeout(exploreTimerRef.current)
      }
    },
    [],
  )

  return { isExploringLine, stopExplorePlayback, exploreLineSlowly }
}
