import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { graphPaths } from '../lib/eval-graph'

interface EvalGraphProps {
  winPcts: readonly number[]
  currentPly: number
  onSelect: (ply: number) => void
  interactive?: boolean
  /** Pulsa o ponto da posição atual — usado no loading pra sinalizar análise ao vivo. */
  pulse?: boolean
  /** Fronteiras de fase (plis finais de Abertura/Meio-jogo) p/ desenhar faixas de fundo. */
  phases?: { openingEnd: number; middlegameEnd: number }
}

const HEIGHT = 124
const PLOT_TOP = 0
const PLOT_BOTTOM = 0
const PLOT_HEIGHT = HEIGHT
const MIDLINE_Y = PLOT_TOP + PLOT_HEIGHT / 2
const PLOT_RADIUS = 8

type PhaseBand = {
  key: 'opening' | 'middlegame' | 'endgame'
  x1: number
  x2: number
  opacity: number
}

function graphY(winPct: number): number {
  return PLOT_TOP + (1 - winPct / 100) * PLOT_HEIGHT
}

export default function EvalGraph({
  winPcts,
  currentPly,
  onSelect,
  interactive = true,
  pulse = false,
  phases,
}: EvalGraphProps) {
  const id = useId()
  const plotId = `${id}-plot`
  const whiteHalfId = `${id}-white`
  const blackHalfId = `${id}-black`
  const ref = useRef<HTMLDivElement>(null)
  const [w, setW] = useState(0)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver((entries) => {
      const width = entries[0].contentRect.width
      if (width > 0) setW(width)
    })
    ro.observe(el)
    setW(el.clientWidth)
    return () => ro.disconnect()
  }, [])

  const geometry = useMemo(() => {
    const n = winPcts.length
    const x = (i: number) => (n <= 1 ? 0 : (i / (n - 1)) * w)
    const ready = w > 0 && n >= 2
    const { linePath, areaPath } = ready
      ? graphPaths(winPcts, w, PLOT_HEIGHT)
      : { linePath: '', areaPath: '' }
    const bands: PhaseBand[] = phases
      ? [
          {
            key: 'opening',
            x1: x(0),
            x2: x(phases.openingEnd),
            opacity: 0.035,
          },
          {
            key: 'middlegame',
            x1: x(phases.openingEnd),
            x2: x(phases.middlegameEnd),
            opacity: 0.06,
          },
          {
            key: 'endgame',
            x1: x(phases.middlegameEnd),
            x2: x(n - 1),
            opacity: 0.035,
          },
        ]
      : []
    return { n, x, ready, linePath, areaPath, bands }
  }, [phases, w, winPcts])

  const { n, x, ready, linePath, areaPath, bands } = geometry

  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (n <= 1 || e.detail === 0) return
    const rect = e.currentTarget.getBoundingClientRect()
    const ratio = (e.clientX - rect.left) / rect.width
    const ply = Math.round(ratio * (n - 1))
    onSelect(Math.max(0, Math.min(n - 1, ply)))
  }

  const cx = x(currentPly)
  const currentValue = winPcts[Math.max(0, Math.min(n - 1, currentPly))]
  const cy = graphY(currentValue ?? 50)

  return (
    <div className='eval-timeline min-w-0'>
      <div className='mb-3 flex flex-wrap items-center justify-between gap-2'>
        <div className='flex items-center gap-3 text-[11px] text-ink-dim'>
          <span className='inline-flex items-center gap-1.5'>
            <span className='eval-timeline-side eval-timeline-side-white' />
            Brancas
          </span>
          <span className='inline-flex items-center gap-1.5'>
            <span className='eval-timeline-side eval-timeline-side-black' />
            Pretas
          </span>
        </div>
        <span className='rounded-lg border border-edge-soft bg-panel-2/70 px-2 py-1 font-mono text-[10px] tabular-nums text-ink-dim'>
          {currentPly === 0 ? 'Posição inicial' : `Posição ${currentPly}`}
        </span>
      </div>
      <div
        ref={ref}
        className='min-h-[106px] w-full min-w-0 sm:min-h-[126px] lg:min-h-[138px]'
      >
        {ready && (
          <button
            type='button'
            onClick={handleClick}
            disabled={!interactive}
            className='eval-timeline-plot block w-full cursor-crosshair select-none overflow-hidden rounded-xl border border-edge outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-default'
            aria-label={
              interactive
                ? 'Gráfico de avaliação das brancas. Clique ou use as setas, Home e End para navegar entre posições.'
                : 'Gráfico de avaliação das brancas. Análise em andamento.'
            }
            onKeyDown={(event) => {
              let ply: number
              switch (event.key) {
                case 'ArrowLeft':
                  ply = currentPly - 1
                  break
                case 'ArrowRight':
                  ply = currentPly + 1
                  break
                case 'Home':
                  ply = 0
                  break
                case 'End':
                  ply = n - 1
                  break
                default:
                  return
              }
              event.preventDefault()
              event.stopPropagation()
              onSelect(Math.max(0, Math.min(n - 1, ply)))
            }}
          >
            <svg
              width={w}
              height={HEIGHT}
              viewBox={`0 0 ${w} ${HEIGHT}`}
              preserveAspectRatio='none'
              role='img'
              aria-hidden='true'
              className='block h-[104px] w-full sm:h-[124px] lg:h-[136px]'
            >
              <defs>
                <clipPath id={plotId}>
                  <rect
                    x={0}
                    y={0}
                    width={w}
                    height={HEIGHT}
                    rx={PLOT_RADIUS}
                  />
                </clipPath>
                <clipPath id={whiteHalfId}>
                  <rect x={0} y={PLOT_TOP} width={w} height={PLOT_HEIGHT / 2} />
                </clipPath>
                <clipPath id={blackHalfId}>
                  <rect
                    x={0}
                    y={MIDLINE_Y}
                    width={w}
                    height={PLOT_HEIGHT / 2}
                  />
                </clipPath>
              </defs>

              <g clipPath={`url(#${plotId})`}>
                {bands.map((b) => (
                  <rect
                    key={b.key}
                    x={Math.min(b.x1, b.x2)}
                    y={0}
                    width={Math.max(0, b.x2 - b.x1)}
                    height={HEIGHT}
                    fill='var(--color-foreground)'
                    opacity={b.opacity}
                  />
                ))}

                {[25, 50, 75].map((pct) => (
                  <line
                    key={pct}
                    x1={0}
                    y1={graphY(pct)}
                    x2={w}
                    y2={graphY(pct)}
                    stroke='var(--color-border)'
                    strokeWidth={pct === 50 ? 1 : 0.75}
                    strokeDasharray={pct === 50 ? undefined : '2 4'}
                  />
                ))}

                {bands.slice(0, -1).map((b) => (
                  <line
                    key={`${b.key}-boundary`}
                    x1={b.x2}
                    y1={PLOT_TOP}
                    x2={b.x2}
                    y2={HEIGHT - PLOT_BOTTOM}
                    stroke='var(--color-border)'
                    strokeWidth={1}
                    strokeDasharray='3 4'
                  />
                ))}

                <path
                  d={areaPath}
                  fill='var(--evalgraph-white-fill)'
                  clipPath={`url(#${whiteHalfId})`}
                />
                <path
                  d={areaPath}
                  fill='var(--evalgraph-black-fill)'
                  clipPath={`url(#${blackHalfId})`}
                />

                <path
                  d={linePath}
                  fill='none'
                  stroke='var(--evalgraph-white-line)'
                  strokeWidth={2}
                  strokeLinejoin='round'
                  strokeLinecap='round'
                  clipPath={`url(#${whiteHalfId})`}
                />
                <path
                  d={linePath}
                  fill='none'
                  stroke='var(--evalgraph-black-line)'
                  strokeWidth={2}
                  strokeLinejoin='round'
                  strokeLinecap='round'
                  clipPath={`url(#${blackHalfId})`}
                />

                <line
                  x1={cx}
                  y1={PLOT_TOP}
                  x2={cx}
                  y2={HEIGHT - PLOT_BOTTOM}
                  stroke='var(--color-ring)'
                  strokeWidth={1}
                />
                {Number.isFinite(currentValue) && (
                  <circle
                    cx={cx}
                    cy={cy}
                    r={9}
                    fill='var(--ink)'
                    opacity={0.08}
                  />
                )}
                {Number.isFinite(currentValue) && (
                  <circle
                    cx={cx}
                    cy={cy}
                    r={4}
                    fill='var(--color-background)'
                    stroke={
                      cy <= MIDLINE_Y
                        ? 'var(--evalgraph-white-line)'
                        : 'var(--evalgraph-black-line)'
                    }
                    strokeWidth={1.75}
                    className={pulse ? 'eval-graph-tip' : undefined}
                  />
                )}
              </g>
            </svg>
          </button>
        )}
      </div>
      <div className='mt-2.5 flex items-center justify-between gap-2 text-[10px] text-ink-dim'>
        <span>Início</span>
        <span className='font-mono tabular-nums'>
          {Number.isFinite(currentValue)
            ? `${Math.round(currentValue)}% · avaliação das brancas`
            : 'Aguardando avaliação'}
        </span>
        <span>{pulse ? 'Analisando…' : 'Fim'}</span>
      </div>
    </div>
  )
}
