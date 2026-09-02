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
        // A disabled control has to look disabled. The `Button` says so with
        // opacity and hands forced colours the one colour an operating system
        // reserves for it; these do the same, so the state is carried the same
        // way everywhere rather than left to whatever the browser does.
        'disabled:opacity-50 forced-colors:disabled:text-[color:GrayText] forced-colors:disabled:opacity-100',
        className,
      )}
      {...rest}
    >
      {children}
    </select>
  )
}
