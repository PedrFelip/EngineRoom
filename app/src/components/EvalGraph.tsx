import { useEffect, useRef, useState } from 'react'

interface EvalGraphProps {
  winPcts: number[]
  currentPly: number
  onSelect: (ply: number) => void
}

const HEIGHT = 96

export default function EvalGraph({
  winPcts,
  currentPly,
  onSelect,
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
  const mid = HEIGHT / 2
  const x = (i: number) => (n <= 1 ? 0 : (i / (n - 1)) * w)
  const y = (wp: number) => (1 - wp / 100) * HEIGHT

  const ready = w > 0 && n >= 2
  const curvePts = ready
    ? winPcts.map((wp, i) => `${x(i).toFixed(1)},${y(wp).toFixed(1)}`)
    : []
  const linePath = ready ? `M ${curvePts.join(' L ')}` : ''
  const ribbonPath = ready
    ? `M ${x(0).toFixed(1)},${mid} L ${curvePts.join(' L ')} L ${x(n - 1).toFixed(1)},${mid} Z`
    : ''

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
    <div ref={ref} className='w-full'>
      {ready && (
        <button
          type='button'
          onClick={handleClick}
          className='block w-full cursor-pointer select-none'
          aria-label='Gráfico de avaliação'
        >
          <svg
            width={w}
            height={HEIGHT}
            role='img'
            aria-hidden='true'
            className='block'
          >
            <defs>
              <clipPath id='eval-top'>
                <rect x={0} y={0} width={w} height={mid} />
              </clipPath>
              <clipPath id='eval-bottom'>
                <rect x={0} y={mid} width={w} height={mid} />
              </clipPath>
            </defs>

            <line
              x1={0}
              y1={mid}
              x2={w}
              y2={mid}
              stroke='var(--color-edge)'
              strokeWidth={1}
            />

            <path d={ribbonPath} fill='#e9e6e0' clipPath='url(#eval-top)' />
            <path d={ribbonPath} fill='#3c3935' clipPath='url(#eval-bottom)' />

            <path
              d={linePath}
              fill='none'
              stroke='var(--color-ink-dim)'
              strokeWidth={1.5}
              strokeLinejoin='round'
              strokeLinecap='round'
            />

            <line
              x1={cx}
              y1={0}
              x2={cx}
              y2={HEIGHT}
              stroke='var(--color-brand)'
              strokeWidth={1.5}
            />
            <circle cx={cx} cy={cy} r={3.5} fill='var(--color-brand)' />
          </svg>
        </button>
      )}
    </div>
  )
}
