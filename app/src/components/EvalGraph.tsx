import { useEffect, useRef, useState } from 'react'

interface EvalGraphProps {
  winPcts: number[]
  currentPly: number
  onSelect: (ply: number) => void
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
  label: string
  compactLabel: string
  x1: number
  x2: number
  opacity: number
}

function smoothedPath(
  points: ReadonlyArray<readonly [number, number]>,
): string {
  if (points.length < 2) return ''
  if (points.length === 2) {
    return `M ${points[0][0].toFixed(1)},${points[0][1].toFixed(1)} L ${points[1][0].toFixed(1)},${points[1][1].toFixed(1)}`
  }

  let path = `M ${points[0][0].toFixed(1)},${points[0][1].toFixed(1)}`
  for (let i = 1; i < points.length - 1; i++) {
    const [x0, y0] = points[i]
    const [x1, y1] = points[i + 1]
    const mx = (x0 + x1) / 2
    const my = (y0 + y1) / 2
    path += ` Q ${x0.toFixed(1)},${y0.toFixed(1)} ${mx.toFixed(1)},${my.toFixed(1)}`
  }
  const [lastX, lastY] = points[points.length - 1]
  return `${path} L ${lastX.toFixed(1)},${lastY.toFixed(1)}`
}

export default function EvalGraph({
  winPcts,
  currentPly,
  onSelect,
  pulse = false,
  phases,
}: EvalGraphProps) {
  const ref = useRef<HTMLDivElement>(null)
  const [w, setW] = useState(0)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver((entries) =>
      setW(entries[0].contentRect.width),
    )
    ro.observe(el)
    setW(el.clientWidth)
    return () => ro.disconnect()
  }, [])

  const n = winPcts.length
  const x = (i: number) => (n <= 1 ? 0 : (i / (n - 1)) * w)
  const y = (wp: number) => PLOT_TOP + (1 - wp / 100) * PLOT_HEIGHT

  const ready = w > 0 && n >= 2
  const points = ready ? winPcts.map((wp, i) => [x(i), y(wp)] as const) : []
  const linePath = ready ? smoothedPath(points) : ''
  const areaPath = ready
    ? `${linePath} L ${x(n - 1).toFixed(1)},${MIDLINE_Y.toFixed(1)} L ${x(0).toFixed(1)},${MIDLINE_Y.toFixed(1)} Z`
    : ''

  const bands: PhaseBand[] = phases
    ? [
        {
          key: 'opening',
          label: 'Abertura',
          compactLabel: 'Abr.',
          x1: x(0),
          x2: x(phases.openingEnd),
          opacity: 0.035,
        },
        {
          key: 'middlegame',
          label: 'Meio-jogo',
          compactLabel: 'Meio',
          x1: x(phases.openingEnd),
          x2: x(phases.middlegameEnd),
          opacity: 0.06,
        },
        {
          key: 'endgame',
          label: 'Final',
          compactLabel: 'Final',
          x1: x(phases.middlegameEnd),
          x2: x(n - 1),
          opacity: 0.035,
        },
      ]
    : []

  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (n <= 1) return
    const rect = e.currentTarget.getBoundingClientRect()
    const ratio = (e.clientX - rect.left) / rect.width
    const ply = Math.round(ratio * (n - 1))
    onSelect(Math.max(0, Math.min(n - 1, ply)))
  }

  const cx = x(currentPly)
  const cy = y(winPcts[Math.max(0, Math.min(n - 1, currentPly))] ?? 50)

  return (
    <div ref={ref} className='w-full min-w-0'>
      {ready && (
        <>
          <button
            type='button'
            onClick={handleClick}
            className='eval-graph block w-full cursor-pointer select-none overflow-hidden rounded-[calc(var(--radius)+2px)] border border-border/70 bg-card/60 shadow-sm outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background'
            aria-label='Gráfico de avaliação. Clique para navegar até um lance.'
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
                <clipPath id='eval-graph-plot'>
                  <rect
                    x={0}
                    y={0}
                    width={w}
                    height={HEIGHT}
                    rx={PLOT_RADIUS}
                  />
                </clipPath>
                <clipPath id='eval-graph-white-half'>
                  <rect x={0} y={PLOT_TOP} width={w} height={PLOT_HEIGHT / 2} />
                </clipPath>
                <clipPath id='eval-graph-black-half'>
                  <rect
                    x={0}
                    y={MIDLINE_Y}
                    width={w}
                    height={PLOT_HEIGHT / 2}
                  />
                </clipPath>
              </defs>

              <g clipPath='url(#eval-graph-plot)'>
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
                    y1={y(pct)}
                    x2={w}
                    y2={y(pct)}
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
                  clipPath='url(#eval-graph-white-half)'
                />
                <path
                  d={areaPath}
                  fill='var(--evalgraph-black-fill)'
                  clipPath='url(#eval-graph-black-half)'
                />

                <path
                  d={linePath}
                  fill='none'
                  stroke='var(--evalgraph-white-line)'
                  strokeWidth={1.75}
                  strokeLinejoin='round'
                  strokeLinecap='round'
                  clipPath='url(#eval-graph-white-half)'
                />
                <path
                  d={linePath}
                  fill='none'
                  stroke='var(--evalgraph-black-line)'
                  strokeWidth={1.75}
                  strokeLinejoin='round'
                  strokeLinecap='round'
                  clipPath='url(#eval-graph-black-half)'
                />

                <line
                  x1={cx}
                  y1={PLOT_TOP}
                  x2={cx}
                  y2={HEIGHT - PLOT_BOTTOM}
                  stroke='var(--color-ring)'
                  strokeWidth={1}
                />
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
              </g>
            </svg>
          </button>
          {bands.length > 0 ? (
            <div className='mt-1.5 flex h-5 overflow-hidden rounded-[calc(var(--radius)-3px)] border border-border/70 bg-muted/40 px-0.5 text-[8px] font-medium tracking-wide text-muted-foreground uppercase sm:text-[9px]'>
              {bands.map((band) => (
                <span
                  key={band.key}
                  className='flex min-w-0 items-center justify-center truncate border-r border-border/70 last:border-r-0'
                  style={{
                    flexGrow: Math.max(0, band.x2 - band.x1),
                    flexBasis: 0,
                  }}
                >
                  <span className='sm:hidden'>{band.compactLabel}</span>
                  <span className='hidden sm:inline'>{band.label}</span>
                </span>
              ))}
            </div>
          ) : null}
        </>
      )}
    </div>
  )
}
