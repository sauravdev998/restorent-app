import type { ComponentPropsWithoutRef } from 'react'

import { cn } from './cn'
import { useFieldControl } from './field-context'

export type SelectProps = Omit<ComponentPropsWithoutRef<'select'>, 'id'>

/**
 * A choice control, always used through `Field`.
 *
 * A real `<select>`, not a listbox rebuilt out of divs. The native control
 * already has the keyboard behaviour, the screen reader behaviour, and the
 * platform picker a phone opens as a full height wheel, which is exactly what a
 * waiter wants one handed. A custom one would be more work and less usable.
 */
export function Select({ className, children, ...rest }: SelectProps) {
  const field = useFieldControl()

  return (
    <select
      id={field.id}
      aria-describedby={field.describedBy}
      aria-invalid={field.invalid || undefined}
      required={field.required}
      className={cn(
        'border-line target-h h-9 w-full rounded-md border-input bg-background px-3 text-sm text-foreground',
        'aria-invalid:border-status-late',
        className,
      )}
      {...rest}
    >
      {children}
    </select>
  )
}
