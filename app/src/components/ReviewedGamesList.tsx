import { resultLabel } from "../lib/pgn";
import { ENGINE_TIERS, type GameSummary } from "../types";

interface Props {
  games: GameSummary[];
  onOpen: (id: number) => void;
  onDelete: (id: number) => void;
  onReanalyze: (id: number) => void;
}

/** "2026-07-17 20:00:00" (UTC do SQLite) → "17/07 17:00" (local). */
function formatDate(createdAt: string): string {
  const d = new Date(createdAt.replace(" ", "T") + "Z");
  if (Number.isNaN(d.getTime())) return createdAt;
  const dd = String(d.getDate()).padStart(2, "0");
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const mi = String(d.getMinutes()).padStart(2, "0");
  return `${dd}/${mm} ${hh}:${mi}`;
}

function tierLabel(game: GameSummary): string {
  const tier = ENGINE_TIERS.find((t) => t.id === game.engineTier);
  return tier ? tier.label : `d${game.depth}`;
}

export default function ReviewedGamesList({ games, onOpen, onDelete, onReanalyze }: Props) {
  return (
    <section className="w-full min-w-0 flex-1">
      <h2 className="mb-3 text-sm font-semibold tracking-wide text-ink-dim uppercase">
        Partidas analisadas
      </h2>
      <ul className="flex max-h-[32rem] flex-col gap-2 overflow-y-auto pr-1">
        {games.map((g) => (
          <li key={g.id}>
            <div
              role="button"
              tabIndex={0}
              onClick={() => onOpen(g.id)}
              onKeyDown={(e) => e.key === "Enter" && onOpen(g.id)}
              className="group flex w-full cursor-pointer items-center gap-3 rounded-xl border border-edge bg-panel/80 px-4 py-3 text-left transition hover:border-brand/60 hover:bg-panel-2"
            >
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-baseline gap-x-2 truncate text-sm">
                  <span className="font-semibold text-ink">{g.white}</span>
                  <span className="text-ink-faint">vs</span>
                  <span className="font-semibold text-ink">{g.black}</span>
                  <span className="text-ink-dim">·</span>
                  <span className="text-good">{resultLabel(g.result)}</span>
                </div>
                <div className="mt-0.5 flex flex-wrap items-center gap-x-2 text-xs text-ink-faint">
                  <span>
                    prec. {Math.round(g.accuracyWhite)}% / {Math.round(g.accuracyBlack)}%
                  </span>
                  <span className="text-ink-faint/60">·</span>
                  <span>{Math.ceil(g.plies / 2)} lances</span>
                  <span className="text-ink-faint/60">·</span>
                  <span>
                    {tierLabel(g)} d{g.depth}
                  </span>
                  <span className="text-ink-faint/60">·</span>
                  <span>{formatDate(g.createdAt)}</span>
                </div>
              </div>

              <div className="flex items-center gap-1 opacity-0 transition group-hover:opacity-100">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onReanalyze(g.id);
                  }}
                  className="rounded-lg p-1.5 text-ink-dim transition hover:bg-panel-3 hover:text-ink"
                  aria-label="Reanalisar com outras configurações"
                  title="Reanalisar com outras configurações"
                >
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="23 4 23 10 17 10" />
                    <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
                  </svg>
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(g.id);
                  }}
                  className="rounded-lg p-1.5 text-ink-dim transition hover:bg-blunder/15 hover:text-blunder"
                  aria-label="Excluir do histórico"
                  title="Excluir do histórico"
                >
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                </button>
              </div>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
