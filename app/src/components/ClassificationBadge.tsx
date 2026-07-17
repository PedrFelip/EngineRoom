import type { Classification } from "../types";
import { CLASSIFICATION_LABELS } from "../lib/scoring";

const BADGE_COLOR: Record<Classification, string> = {
  livro: "bg-book",
  melhor: "bg-best",
  excelente: "bg-excellent",
  bom: "bg-good",
  imprecisao: "bg-mistake",
  erro: "bg-erro",
  blunder: "bg-blunder",
};

interface ClassificationBadgeProps {
  classification: Classification;
}

export default function ClassificationBadge({
  classification,
}: ClassificationBadgeProps) {
  const label = CLASSIFICATION_LABELS[classification];
  return (
    <span
      className={`inline-block h-2.5 w-2.5 shrink-0 rounded-full ${BADGE_COLOR[classification]}`}
      title={label}
      aria-label={label}
    />
  );
}
