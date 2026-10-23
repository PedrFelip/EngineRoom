interface EvalBarProps {
  winPct: number
  orientation?: 'white' | 'black'
  label?: string
}

export default function EvalBar({
  winPct,
  orientation = 'white',
  label,
}: EvalBarProps) {
  const clamped = Math.max(0, Math.min(100, winPct))
  const whiteAtBottom = orientation === 'white'
  const whiteAdv = clamped >= 50
  const labelAtBottom = whiteAdv === whiteAtBottom
  const description = label
    ? `Avaliação pelas brancas: ${label}`
    : 'Avaliação da posição'

  return (
    <div className='eval-meter' title={description}>
      <meter
        className='sr-only'
        min={0}
        max={100}
        value={clamped}
        aria-label='Avaliação da posição pelas brancas'
        aria-valuetext={description}
      />
      <div className='eval-meter-track' aria-hidden='true'>
        <div
          className='eval-meter-white'
          data-bottom={whiteAtBottom}
          style={{ height: `${clamped}%` }}
        />
        <div className='eval-meter-midpoint' />
        <div
          className='eval-meter-position'
          style={{
            top: `${whiteAtBottom ? 100 - clamped : clamped}%`,
          }}
        />
        {label ? (
          <span
            className='eval-meter-label'
            data-bottom={labelAtBottom}
            data-side={whiteAdv ? 'white' : 'black'}
          >
            {label}
          </span>
        ) : null}
      </div>
    </div>
  )
}
