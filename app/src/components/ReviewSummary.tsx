import type { Classification, ReviewResult } from "../types";
import { CLASSIFICATION_LABELS } from "../lib/scoring";

const ORDER: Classification[] = [
  "livro",
  "melhor",
  "excelente",
  "bom",
  "imprecisao",
  "erro",
  "blunder",
];

const BADGE_COLOR: Record<Classification, string> = {
  livro: "bg-book",
  melhor: "bg-best",
  excelente: "bg-excellent",
  bom: "bg-good",
  imprecisao: "bg-mistake",
  erro: "bg-erro",
  blunder: "bg-blunder",
};

function countBy(moves: { classification: Classification }[]): number[] {
  return ORDER.map((c) => moves.filter((m) => m.classification === c).length);
}

interface ReviewSummaryProps {
  result: ReviewResult;
}

export default function ReviewSummary({ result }: ReviewSummaryProps) {
  const whiteMoves = result.moves.filter((m) => m.color === "w");
  const blackMoves = result.moves.filter((m) => m.color === "b");
  const whiteCounts = countBy(whiteMoves);
  const blackCounts = countBy(blackMoves);

  return (
    <div className="rounded-xl border border-edge bg-panel-2/60 p-4">
      <div className="grid grid-cols-2 gap-3">
        <SideAccuracy label="Brancas" value={result.accuracy.white} />
        <SideAccuracy label="Pretas" value={result.accuracy.black} />
      </div>
      <div className="mt-4 space-y-1.5">
        {ORDER.map((c, i) => (
          <div key={c} className="grid grid-cols-[1fr_auto_auto] items-center gap-3 text-sm">
            <span className="flex items-center gap-2 text-ink-dim">
              <span className={`h-2.5 w-2.5 rounded-full ${BADGE_COLOR[c]}`} />
              {CLASSIFICATION_LABELS[c]}
            </span>
            <span className="w-8 text-right font-mono tabular-nums text-ink">
              {whiteCounts[i]}
            </span>
            <span className="w-8 text-right font-mono tabular-nums text-ink">
              {blackCounts[i]}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function SideAccuracy({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg bg-panel-3/50 p-3 text-center">
      <div className="text-xs uppercase tracking-wide text-ink-faint">{label}</div>
      <div className="mt-1 font-mono text-2xl font-bold tabular-nums text-brand">
        {value.toFixed(1)}%
      </div>
    </div>
  );
}
