import { useId } from 'react'
import { cn } from '@/lib/utils'
import { Switch, type SwitchProps } from './switch'

type SwitchFieldProps = Omit<
  SwitchProps,
  'children' | 'aria-label' | 'aria-labelledby'
> & {
  label: string
  description?: string
  className?: string
}

/** A labeled setting with a shared hit area and an independently announced hint. */
export function SwitchField({
  label,
  description,
  id,
  className,
  disabled,
  'aria-describedby': describedBy,
  ...props
}: SwitchFieldProps) {
  const generatedId = useId()
  const controlId = id ?? generatedId
  const labelId = `${controlId}-label`
  const descriptionId = `${controlId}-description`
  return (
    <label
      htmlFor={controlId}
      className={cn(
        'flex min-h-11 items-center justify-between gap-3',
        disabled ? 'cursor-not-allowed' : 'cursor-pointer',
        className,
      )}
    >
      <span className='min-w-0 flex-1'>
        <span id={labelId} className='block text-sm font-medium text-ink'>
          {label}
        </span>
        {description && (
          <span
            id={descriptionId}
            className='mt-0.5 block text-xs text-ink-faint'
          >
            {description}
          </span>
        )}
      </span>
      <Switch
        {...props}
        id={controlId}
        disabled={disabled}
        aria-labelledby={labelId}
        aria-describedby={
          [description ? descriptionId : undefined, describedBy]
            .filter(Boolean)
            .join(' ') || undefined
        }
      />
    </label>
  )
}
