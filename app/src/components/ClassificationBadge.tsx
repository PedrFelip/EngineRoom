import {
  BookOpen,
  CircleAlert,
  CircleX,
  type LucideIcon,
  Sparkles,
  Star,
  Target,
  ThumbsUp,
  TriangleAlert,
} from 'lucide-react'
import { CLASSIFICATION_LABELS } from '../lib/scoring'
import type { Classification } from '../types'

const BADGE_COLOR: Record<Classification, string> = {
  brilhante: 'bg-great',
  livro: 'bg-book',
  melhor: 'bg-best',
  excelente: 'bg-excellent',
  bom: 'bg-good',
  imprecisao: 'bg-mistake',
  erro: 'bg-erro',
  blunder: 'bg-blunder',
}

const BADGE_GLYPH: Record<Classification, LucideIcon> = {
  brilhante: Sparkles,
  livro: BookOpen,
  melhor: Target,
  excelente: Star,
  bom: ThumbsUp,
  imprecisao: CircleAlert,
  erro: TriangleAlert,
  blunder: CircleX,
}

interface ClassificationBadgeProps {
  classification: Classification
}

/** Quadrado colorido com glifo próprio por classificação (estilo notação Lichess). */
export function ClassGlyph({
  classification,
}: {
  classification: Classification
}) {
  const Icon = BADGE_GLYPH[classification]
  const label = CLASSIFICATION_LABELS[classification]
  return (
    <span
      role='img'
      className={`inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-[5px] text-bg ${BADGE_COLOR[classification]}`}
      title={label}
      aria-label={label}
    >
      <Icon size={10} strokeWidth={3} aria-hidden='true' />
    </span>
  )
}

export default function ClassificationBadge({
  classification,
}: ClassificationBadgeProps) {
  return <ClassGlyph classification={classification} />
}
