import {
  ENGINE_TIERS,
  type EngineMode,
  type EngineTier,
  type EngineTierId,
} from '../types'

interface Props {
  mode: EngineMode
  tierId: EngineTierId
  movetimeMs: number
  lines: number
  plies: number
  onModeChange: (mode: EngineMode) => void
  onTierChange: (tier: EngineTier) => void
  onMovetimeChange: (ms: number) => void
  onLinesChange: (lines: number) => void
}

const MIN_LINES = 1
const MAX_LINES = 5
const MIN_SECONDS = 1
const MAX_SECONDS = 30
const DEFAULT_TIME_MS = 5000

/**
 * Estimativa de tempo total (em segundos) para uma análise por tempo fixo.
 * Usa `plies + 1` posições (a inicial também é avaliada). É um teto, não uma
 * média — posições terminais (mate/afogamento) pulam a engine.
 */
export function estimateTimeSeconds(plies: number, movetimeMs: number): number {
  return Math.max(0, (plies + 1) * movetimeMs) / 1000
}

function formatEta(seconds: number): string {
  if (seconds < 60) return `~${Math.round(seconds)}s`
  const min = Math.floor(seconds / 60)
  const sec = Math.round(seconds % 60)
  return `~${min}min${sec > 0 ? ` ${sec}s` : ''}`
}

export default function EngineTierSelector({
  mode,
  tierId,
  movetimeMs,
  lines,
  plies,
  onModeChange,
  onTierChange,
  onMovetimeChange,
  onLinesChange,
}: Props) {
  const activeIndex = ENGINE_TIERS.findIndex((t) => t.id === tierId)
  const active = ENGINE_TIERS[activeIndex] ?? ENGINE_TIERS[1]
  const linesPct = ((lines - MIN_LINES) / (MAX_LINES - MIN_LINES)) * 100
  const seconds = Math.max(
    MIN_SECONDS,
    Math.min(MAX_SECONDS, Math.round(movetimeMs / 1000)),
  )
  const timePct = ((seconds - MIN_SECONDS) / (MAX_SECONDS - MIN_SECONDS)) * 100
  const eta = estimateTimeSeconds(plies, seconds * 1000)

  return (
    <div className='rounded-xl border border-edge bg-panel-2/60 p-5'>
      {/* Mode toggle */}
      <div className='mb-4 flex items-baseline justify-between'>
        <div>
          <h3 className='text-sm font-semibold uppercase tracking-wide text-ink-dim'>
            Modo de análise
          </h3>
          <p className='mt-0.5 text-xs text-ink-faint'>
            Profundidade fixa ou tempo por lance
          </p>
        </div>
      </div>
      <div className='mb-5 grid grid-cols-2 gap-1 rounded-lg bg-panel-3/60 p-1'>
        {(['depth', 'time'] as const).map((m) => (
          <button
            key={m}
            type='button'
            onClick={() => onModeChange(m)}
            className={`rounded-md px-3 py-1.5 text-sm font-semibold transition ${
              mode === m
                ? 'bg-brand text-bg shadow'
                : 'text-ink-dim hover:text-ink'
            }`}
          >
            {m === 'depth' ? 'Profundidade' : 'Tempo'}
          </button>
        ))}
      </div>

      {/* Depth slider */}
      {mode === 'depth' ? (
        <>
          <div className='mb-4 flex items-baseline justify-between'>
            <h3 className='text-sm font-semibold uppercase tracking-wide text-ink-dim'>
              Qualidade da engine
            </h3>
            <span className='font-mono text-3xl font-bold tabular-nums text-brand'>
              d{active.depth}
            </span>
          </div>
          <input
            type='range'
            min={0}
            max={ENGINE_TIERS.length - 1}
            step={1}
            value={activeIndex}
            onChange={(e) =>
              onTierChange(ENGINE_TIERS[Number(e.currentTarget.value)])
            }
            aria-label='Profundidade'
            className='engine-range w-full'
            style={{
              background: `linear-gradient(to right, var(--color-brand) 0%, var(--color-brand) ${
                (activeIndex / (ENGINE_TIERS.length - 1)) * 100
              }%, var(--color-panel-3) ${
                (activeIndex / (ENGINE_TIERS.length - 1)) * 100
              }%, var(--color-panel-3) 100%)`,
            }}
          />
          <div className='mt-3 grid grid-cols-3 gap-1'>
            {ENGINE_TIERS.map((tier, i) => (
              <button
                key={tier.id}
                type='button'
                onClick={() => onTierChange(tier)}
                className={`rounded-lg px-2 py-2 text-center transition ${
                  i === activeIndex
                    ? 'bg-brand/15 ring-1 ring-brand/50'
                    : 'hover:bg-panel-3/50'
                }`}
              >
                <div
                  className={`text-sm font-semibold ${
                    i === activeIndex ? 'text-brand' : 'text-ink'
                  }`}
                >
                  {tier.label}
                </div>
                <div className='font-mono text-[11px] text-ink-faint'>
                  d{tier.depth}
                </div>
              </button>
            ))}
          </div>
          <p className='mt-3 min-h-[1.25rem] text-center text-xs text-ink-dim'>
            {active.hint}
          </p>
        </>
      ) : (
        <>
          <div className='mb-4 flex items-baseline justify-between'>
            <h3 className='text-sm font-semibold uppercase tracking-wide text-ink-dim'>
              Tempo por lance
            </h3>
            <span className='font-mono text-3xl font-bold tabular-nums text-brand'>
              {seconds}s
            </span>
          </div>
          <input
            type='range'
            min={MIN_SECONDS}
            max={MAX_SECONDS}
            step={1}
            value={seconds}
            onChange={(e) =>
              onMovetimeChange(Number(e.currentTarget.value) * 1000)
            }
            aria-label='Segundos por lance'
            className='engine-range w-full'
            style={{
              background: `linear-gradient(to right, var(--color-brand) 0%, var(--color-brand) ${timePct}%, var(--color-panel-3) ${timePct}%, var(--color-panel-3) 100%)`,
            }}
          />
          <div className='mt-2 flex justify-between font-mono text-[11px] text-ink-faint'>
            <span>{MIN_SECONDS}s</span>
            <span>{MAX_SECONDS}s</span>
          </div>
          <p className='mt-3 min-h-[1.25rem] text-center text-xs text-ink-dim'>
            {plies > 0
              ? `Máximo ${formatEta(eta)} no total · ${plies} lances`
              : 'Máximo por lance (import um PGN para estimar o total)'}
          </p>
        </>
      )}

      <div className='my-5 h-px bg-edge-soft' />

      {/* Lines slider (comum aos dois modos) */}
      <div className='mb-4 flex items-baseline justify-between'>
        <div>
          <h3 className='text-sm font-semibold uppercase tracking-wide text-ink-dim'>
            Linhas de análise
          </h3>
          <p className='mt-0.5 text-xs text-ink-faint'>
            Quantas linhas candidatas a engine retorna por lance
          </p>
        </div>
        <span className='font-mono text-3xl font-bold tabular-nums text-brand'>
          {lines}
        </span>
      </div>

      <input
        type='range'
        min={MIN_LINES}
        max={MAX_LINES}
        step={1}
        value={lines}
        onChange={(e) => onLinesChange(Number(e.currentTarget.value))}
        aria-label='Linhas de análise'
        className='engine-range w-full'
        style={{
          background: `linear-gradient(to right, var(--color-brand) 0%, var(--color-brand) ${linesPct}%, var(--color-panel-3) ${linesPct}%, var(--color-panel-3) 100%)`,
        }}
      />
      <div className='mt-2 flex justify-between font-mono text-[11px] text-ink-faint'>
        {Array.from({ length: MAX_LINES }, (_, i) => i + 1).map((n) => (
          <span key={n} className={n === lines ? 'text-brand' : ''}>
            {n}
          </span>
        ))}
      </div>
      <p className='mt-2 min-h-[1.25rem] text-center text-xs text-ink-dim'>
        {lines === 1
          ? 'Apenas a melhor linha (mais rápido)'
          : `${lines} linhas candidatas por lance`}
      </p>
    </div>
  )
}

export { DEFAULT_TIME_MS }
