import { forwardRef, type InputHTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

type InputProps = InputHTMLAttributes<HTMLInputElement> & {
  /** The surrounding search field owns its border and focus-within ring. */
  embedded?: boolean
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, embedded = false, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        'min-w-0 text-sm text-ink outline-none placeholder:text-ink-faint aria-invalid:border-destructive aria-invalid:focus-visible:ring-destructive/25 disabled:cursor-not-allowed disabled:opacity-50',
        embedded
          ? 'bg-transparent'
          : 'h-10 w-full rounded-[var(--control-radius)] border border-edge bg-panel-2/40 px-3 transition-[border-color,box-shadow] focus-visible:border-brand focus-visible:ring-2 focus-visible:ring-brand/25',
        className,
      )}
      {...props}
    />
  ),
)
Input.displayName = 'Input'
