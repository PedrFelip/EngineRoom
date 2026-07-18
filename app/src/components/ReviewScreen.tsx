import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { ReviewConfig } from "../types";
import { resultLabel } from "../lib/pgn";
import { evalLabel, sideToMoveAtPly } from "../lib/eval-label";
import { useReview } from "../lib/use-review";
import Board from "./Board";
import EvalBar from "./EvalBar";
import EvalGraph from "./EvalGraph";
import MoveList from "./MoveList";
import ReviewSummary from "./ReviewSummary";
import CandidateLines from "./CandidateLines";

interface ReviewScreenProps {
  config: ReviewConfig;
  onExit: () => void;
}

function uciToSquares(uci: string): [string, string] | null {
  if (uci.length < 4) return null;
  return [uci.slice(0, 2), uci.slice(2, 4)];
}

export default function ReviewScreen({ config, onExit }: ReviewScreenProps) {
  const review = useReview(config);
  const { result, status, error, currentPly, orientation } = review;
  const position = result?.positions[currentPly] ?? null;
  const lastMoveUci =
    currentPly > 0 ? result?.moves[currentPly - 1].uci ?? null : null;
  const opening = result?.moves.find((m) => m.eco)?.eco ?? null;

  const evalBarLabel =
    position && result
      ? evalLabel(
          position.cp,
          position.fen,
          sideToMoveAtPly(result.moves, currentPly),
        )
      : undefined;

  const [selectedMultipv, setSelectedMultipv] = useState(1);
  useEffect(() => {
    setSelectedMultipv(1);
  }, [currentPly]);
  const selectedLine =
    position?.lines.find((l) => l.multipv === selectedMultipv) ?? position?.lines[0];

  const bestArrow = useMemo(() => {
    const uci = selectedLine?.pv[0];
    if (!uci) return null;
    const sq = uciToSquares(uci);
    return sq ? { from: sq[0], to: sq[1], brush: "green" as const } : null;
  }, [selectedLine]);

  useEffect(() => {
    if (!result) return;
    const onKey = (e: KeyboardEvent) => {
      switch (e.key) {
        case "ArrowLeft":
          review.prev();
          break;
        case "ArrowRight":
          review.next();
          break;
        case "Home":
          review.first();
          break;
        case "End":
          review.last();
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [result, review.prev, review.next, review.first, review.last]);

  return (
    <div className="mx-auto flex min-h-full max-w-6xl flex-col gap-4 px-4 py-6">
      <header className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-lg font-bold text-ink">
            {config.meta.white}{" "}
            <span className="text-ink-faint">vs</span> {config.meta.black}
          </h1>
          <p className="text-sm text-ink-dim">
            {resultLabel(config.meta.result)} · {Math.ceil(config.meta.plies / 2)} lances ·
            engine d{config.engine.depth}
            {opening ? ` · ${opening.code} ${opening.name}` : ""}
          </p>
        </div>
        <button
          type="button"
          onClick={onExit}
          className="rounded-xl bg-panel-3 px-4 py-2 text-sm font-medium text-ink transition hover:bg-edge"
        >
          ← Nova partida
        </button>
      </header>

      {status === "error" && (
        <div className="rounded-xl border border-blunder/50 bg-blunder/10 p-4 text-sm text-blunder">
          Falha na análise: {error}
        </div>
      )}

      <div className="grid flex-1 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
        <div className="flex flex-col gap-3">
          <div className="flex items-stretch gap-2">
            <EvalBar
              winPct={position?.winPct ?? 50}
              orientation={orientation}
              label={evalBarLabel}
            />
            <div className="min-w-0 flex-1">
              {position ? (
                <Board
                  fen={position.fen}
                  orientation={orientation}
                  lastMove={lastMoveUci ? uciToSquares(lastMoveUci) : null}
                  arrows={bestArrow ? [bestArrow] : []}
                />
              ) : (
                <div className="flex aspect-square w-full items-center justify-center rounded-lg border border-edge bg-panel-2/60 text-ink-dim">
                  {status === "running" ? "Analisando…" : "—"}
                </div>
              )}
            </div>
          </div>

          {position?.lines?.length ? (
            <CandidateLines
              lines={position.lines}
              selectedMultipv={selectedMultipv}
              onSelect={setSelectedMultipv}
            />
          ) : null}

          <div className="flex items-center justify-center gap-2 rounded-xl border border-edge bg-panel-2/60 p-2">
            <NavBtn onClick={review.first} disabled={!result}>
              ⏮
            </NavBtn>
            <NavBtn onClick={review.prev} disabled={!result || currentPly === 0}>
              ‹
            </NavBtn>
            <NavBtn
              onClick={review.next}
              disabled={!result || currentPly >= (result?.moves.length ?? 0)}
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
          </div>
        </div>

        <aside className="flex flex-col gap-4">
          {result && <ReviewSummary result={result} />}
          {result && (
            <div className="max-h-[50vh] overflow-y-auto rounded-xl border border-edge bg-panel-2/60 p-3">
              <MoveList
                moves={result.moves}
                currentPly={currentPly}
                onSelect={review.goTo}
              />
            </div>
          )}
        </aside>
      </div>

      {result && (
        <div className="rounded-xl border border-edge bg-panel-2/60 p-3">
          <div className="mb-1.5 flex items-center justify-between px-1">
            <span className="text-[11px] font-semibold uppercase tracking-wide text-ink-faint">
              Avaliação
            </span>
            <span className="text-[11px] text-ink-faint">
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
  );
}

function NavBtn({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="rounded-lg px-3 py-1.5 text-lg text-ink-dim transition hover:bg-panel-3/60 disabled:opacity-30 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}
