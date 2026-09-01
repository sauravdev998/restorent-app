import type { ComponentPropsWithoutRef } from 'react'

import { cn } from './cn'
import { useFieldControl } from './field-context'

export type InputProps = Omit<ComponentPropsWithoutRef<'input'>, 'id'>

/**
 * A text control, always used through `Field`.
 *
 * Its type size is `text-sm`, which is 16px on the waiter surface. That is not
 * a coincidence: below 16px, iOS zooms the whole page when the control takes
 * focus, and a waiter who has to pinch back out mid order stops using the app.
 */
export function Input({ className, ...rest }: InputProps) {
  const field = useFieldControl()

  return (
    <input
      id={field.id}
      aria-describedby={field.describedBy}
      aria-invalid={field.invalid || undefined}
      required={field.required}
      className={cn(
        'border-line target-h h-9 w-full rounded-md border-input bg-background px-3 text-sm text-foreground',
        'placeholder:text-muted-foreground',
        'aria-invalid:border-status-late',
        className,
      )}
      {...rest}
    />
  )
}
