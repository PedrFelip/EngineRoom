import { useState } from "react";
import HomePage from "./components/HomePage";
import { resultLabel } from "./lib/pgn";
import type { ReviewConfig } from "./types";

type View = "home" | "review";

export default function App() {
  const [view, setView] = useState<View>("home");
  const [config, setConfig] = useState<ReviewConfig | null>(null);

  if (view === "home" || !config) {
    return (
      <HomePage
        onStart={(cfg) => {
          setConfig(cfg);
          setView("review");
        }}
      />
    );
  }

  return (
    <div className="flex min-h-full flex-col items-center justify-center px-4 py-10">
      <div className="w-full max-w-md rounded-2xl border border-edge bg-panel/80 p-6 text-center shadow-xl shadow-black/30">
        <p className="text-sm uppercase tracking-wide text-ink-faint">Partida carregada</p>
        <h2 className="mt-1 text-xl font-bold text-ink">
          {config.meta.white} <span className="text-ink-faint">vs</span> {config.meta.black}
        </h2>
        <p className="mt-1 text-sm text-ink-dim">
          {resultLabel(config.meta.result)} · {Math.ceil(config.meta.plies / 2)} lances
        </p>
        <p className="mt-4 rounded-lg bg-panel-2/60 px-3 py-2 text-sm text-ink-dim">
          Tela de revisão em construção — engine d{config.engine.depth}
        </p>
        <button
          type="button"
          onClick={() => setView("home")}
          className="mt-5 rounded-xl bg-panel-3 px-4 py-2.5 text-sm font-medium text-ink transition hover:bg-edge"
        >
          ← Nova partida
        </button>
      </div>
    </div>
  );
}
