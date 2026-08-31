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
  // Rótulo ancorado no extremo do lado vencedor, cor invertida p/ contraste.
  const labelAnchor = whiteAdv
    ? whiteAtBottom
      ? { bottom: 2 }
      : { top: 2 }
    : whiteAtBottom
      ? { top: 2 }
      : { bottom: 2 }
  const labelColor = whiteAdv
    ? 'var(--evalbar-label-on-white)'
    : 'var(--evalbar-label-on-black)'
  return (
    <div
      className='relative w-6 shrink-0 self-stretch rounded-[var(--radius)] border border-border p-px shadow-[inset_0_1px_rgb(0_0_0_/_16%)]'
      style={{ backgroundColor: 'var(--evalbar-side-black)' }}
      title={label ? `Vantagem das brancas: ${label}` : undefined}
    >
      <div className='relative h-full overflow-hidden rounded-[calc(var(--radius)-2px)]'>
        <div
          className='absolute left-0 right-0 transition-[height,top,bottom] duration-500 ease-out'
          style={{
            ...(whiteAtBottom
              ? { bottom: 0, height: `${clamped}%` }
              : { top: 0, height: `${clamped}%` }),
            backgroundColor: 'var(--evalbar-side-white)',
          }}
        />
        <div className='pointer-events-none absolute left-1 right-1 top-1/2 z-10 h-px -translate-y-1/2 bg-border/70' />
        {label ? (
          <span
            className='pointer-events-none absolute left-1/2 z-20 -translate-x-1/2 rounded-[3px] px-0.5 py-px font-mono text-[8px] font-semibold leading-none whitespace-nowrap'
            style={{
              ...labelAnchor,
              color: labelColor,
              backgroundColor: whiteAdv
                ? 'color-mix(in srgb, var(--evalbar-side-white) 86%, transparent)'
                : 'color-mix(in srgb, var(--evalbar-side-black) 78%, transparent)',
            }}
          >
            {label}
          </span>
        ) : null}
      </div>
    </div>
  )
}
