import './switch.css'
import { Switch as SwitchPrimitive } from 'radix-ui'
import type * as React from 'react'
import { cn } from '@/lib/utils'

type SwitchProps = React.ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: 'sm' | 'default'
}

function Switch({
  className,
  size = 'default',
  type = 'button',
  ...props
}: SwitchProps) {
  return (
    <SwitchPrimitive.Root
      data-slot='switch'
      data-size={size}
      type={type}
      className={cn(
        'ui-switch inline-flex shrink-0 items-center rounded-full border border-transparent p-0.5 outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=unchecked]:bg-input dark:data-[state=unchecked]:bg-input/80',
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot='switch-thumb'
        className='pointer-events-none block rounded-full bg-background dark:data-[state=checked]:bg-primary-foreground dark:data-[state=unchecked]:bg-foreground'
      />
    </SwitchPrimitive.Root>
  )
}

export { Switch, type SwitchProps }
