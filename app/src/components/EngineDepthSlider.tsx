import { ENGINE_TIERS, type EngineTier, type EngineTierId } from "../types";

interface Props {
  value: EngineTierId;
  onChange: (tier: EngineTier) => void;
}

export default function EngineDepthSlider({ value, onChange }: Props) {
  const activeIndex = ENGINE_TIERS.findIndex((t) => t.id === value);
  const active = ENGINE_TIERS[activeIndex];

  return (
    <div className="rounded-xl border border-edge bg-panel-2/60 p-5">
      <div className="mb-4 flex items-baseline justify-between">
        <div>
          <h3 className="text-sm font-semibold uppercase tracking-wide text-ink-dim">
            Qualidade da engine
          </h3>
          <p className="mt-0.5 text-xs text-ink-faint">Profundidade de análise Stockfish</p>
        </div>
        <span className="font-mono text-3xl font-bold tabular-nums text-brand">
          d{active.depth}
        </span>
      </div>

      <input
        type="range"
        min={0}
        max={ENGINE_TIERS.length - 1}
        step={1}
        value={activeIndex}
        onChange={(e) => onChange(ENGINE_TIERS[Number(e.currentTarget.value)])}
        aria-label="Qualidade da engine"
        className="engine-range w-full"
        style={{
          background: `linear-gradient(to right, var(--color-brand) 0%, var(--color-brand) ${
            (activeIndex / (ENGINE_TIERS.length - 1)) * 100
          }%, var(--color-panel-3) ${
            (activeIndex / (ENGINE_TIERS.length - 1)) * 100
          }%, var(--color-panel-3) 100%)`,
        }}
      />

      <div className="mt-3 grid grid-cols-3 gap-1">
        {ENGINE_TIERS.map((tier, i) => (
          <button
            key={tier.id}
            type="button"
            onClick={() => onChange(tier)}
            className={`rounded-lg px-2 py-2 text-center transition ${
              i === activeIndex
                ? "bg-brand/15 ring-1 ring-brand/50"
                : "hover:bg-panel-3/50"
            }`}
          >
            <div
              className={`text-sm font-semibold ${
                i === activeIndex ? "text-brand" : "text-ink"
              }`}
            >
              {tier.label}
            </div>
            <div className="font-mono text-[11px] text-ink-faint">d{tier.depth}</div>
          </button>
        ))}
      </div>

      <p className="mt-3 min-h-[1.25rem] text-center text-xs text-ink-dim">{active.hint}</p>
    </div>
  );
}
