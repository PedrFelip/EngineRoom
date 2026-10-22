import type { LucideIcon } from 'lucide-react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'

interface Option<Value extends string> {
  id: Value
  label: string
  icon?: LucideIcon
  controls?: string
}

interface Props<Value extends string> {
  label: string
  value: Value
  options: readonly Option<Value>[]
  onChange: (value: Value) => void
  disabled?: boolean
  className?: string
  stackedOnMobile?: boolean
}

export function SegmentedControl<Value extends string>({
  label,
  value,
  options,
  onChange,
  disabled,
  className,
  stackedOnMobile,
}: Props<Value>) {
  return (
    <fieldset
      aria-label={label}
      disabled={disabled}
      className={cn(
        'surface-glass segmented-control flex min-w-0 rounded-2xl border border-edge p-1.5',
        className,
      )}
      style={
        {
          '--tab-count': options.length,
          '--tab-index': options.findIndex((option) => option.id === value),
        } as CSSProperties
      }
    >
      <span className='segmented-indicator' aria-hidden='true' />
      {options.map(({ id, label: optionLabel, icon: Icon, controls }) => (
        <button
          key={id}
          type='button'
          aria-pressed={value === id}
          aria-controls={controls}
          onClick={() => onChange(id)}
          className={cn(
            'segmented-button flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-xl px-2 py-2.5 text-xs font-medium',
            stackedOnMobile &&
              'flex-col px-1 text-[11px] sm:flex-row sm:px-3 sm:text-sm',
          )}
        >
          {Icon && <Icon size={14} aria-hidden='true' />}
          {optionLabel}
        </button>
      ))}
    </fieldset>
  )
}
