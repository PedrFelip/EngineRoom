import { forwardRef, type TextareaHTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

export const Textarea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement>
>(({ className, ...props }, ref) => (
  <textarea
    ref={ref}
    className={cn(
      'w-full min-w-0 resize-y rounded-[var(--control-radius)] border border-edge bg-panel-2/40 p-3 text-sm text-ink outline-none transition-[border-color,box-shadow] placeholder:text-ink-faint aria-invalid:border-destructive aria-invalid:focus-visible:ring-destructive/25 focus-visible:border-brand focus-visible:ring-2 focus-visible:ring-brand/25 disabled:cursor-not-allowed disabled:opacity-50',
      className,
    )}
    {...props}
  />
))
Textarea.displayName = 'Textarea'
