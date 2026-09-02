import { Crown } from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '../ui/button'

export function ReviewNavButton({
  onClick,
  disabled,
  label,
  children,
}: {
  onClick: () => void
  disabled?: boolean
  label: string
  children: ReactNode
}) {
  return (
    <Button
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
      variant='ghost'
      size='icon'
      className='text-ink-dim disabled:opacity-30'
    >
      {children}
    </Button>
  )
}

export function PlayerTag({
  name,
  elo,
  color,
}: {
  name: string
  elo: string | null
  color: 'w' | 'b'
}) {
  return (
    <div className='flex items-center gap-2 px-1 text-sm'>
      <span
        className='flex h-5 w-5 shrink-0 items-center justify-center rounded-full ring-1 ring-edge'
        style={{
          backgroundColor:
            color === 'w' ? 'var(--piece-white-bg)' : 'var(--piece-black-bg)',
          color:
            color === 'w' ? 'var(--piece-white-fg)' : 'var(--piece-black-fg)',
        }}
      >
        <Crown size={11} strokeWidth={2.5} aria-hidden='true' />
      </span>
      <span className='font-medium text-ink'>{name}</span>
      {elo ? (
        <span className='font-mono text-xs text-ink-dim'>({elo})</span>
      ) : null}
    </div>
  )
}
