import {
  BookOpen,
  CircleAlert,
  CircleCheckBig,
  CircleX,
  type LucideIcon,
  Target,
  ThumbsUp,
  TriangleAlert,
} from 'lucide-react'
import { CLASSIFICATION_LABELS } from '../lib/scoring'
import type { Classification } from '../types'

const BADGE_TONE: Record<Classification, string> = {
  livro: 'bg-book text-white',
  melhor: 'bg-best text-white',
  excelente: 'bg-excellent text-white',
  bom: 'bg-good text-white',
  imprecisao: 'bg-mistake text-zinc-950',
  erro: 'bg-erro text-white',
  blunder: 'bg-blunder text-white',
}

const BADGE_GLYPH: Record<Classification, LucideIcon> = {
  livro: BookOpen,
  melhor: Target,
  excelente: CircleCheckBig,
  bom: ThumbsUp,
  imprecisao: CircleAlert,
  erro: TriangleAlert,
  blunder: CircleX,
}

interface ClassificationBadgeProps {
  classification: Classification
}

/** Selo de tom semântico e glifo próprio; a leitura não depende apenas da cor. */
export function ClassGlyph({
  classification,
  size = 'compact',
}: {
  classification: Classification
  size?: 'compact' | 'board'
}) {
  const Icon = BADGE_GLYPH[classification]
  const label = CLASSIFICATION_LABELS[classification]
  const sizeClass =
    size === 'board'
      ? 'h-5 w-5 rounded-md border-2 border-background shadow-md'
      : 'h-4 w-4 rounded-[5px]'
  return (
    <span
      role='img'
      className={`inline-flex shrink-0 items-center justify-center ${sizeClass} ${BADGE_TONE[classification]}`}
      title={label}
      aria-label={label}
    >
      <Icon
        size={size === 'board' ? 12 : 10}
        strokeWidth={3}
        aria-hidden='true'
      />
    </span>
  )
}

export default function ClassificationBadge({
  classification,
}: ClassificationBadgeProps) {
  return <ClassGlyph classification={classification} />
}
