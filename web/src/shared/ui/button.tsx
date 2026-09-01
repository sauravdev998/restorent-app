import { cva, type VariantProps } from 'class-variance-authority'
import type { ComponentPropsWithoutRef, MouseEvent } from 'react'

import { cn } from './cn'

/**
 * `target-h` is the floor that makes the small size honest: `h-8` is 32px for
 * an admin, which is fine, but only 40px on a phone and 64px on a kitchen wall,
 * which are not. The minimum lifts both to the size the surface demands, so
 * there is no size of this button that is too small to hit in the room it is
 * used in.
 */
const button = cva(
  [
    'border-line target-h inline-flex items-center justify-center gap-2 rounded-md',
    'font-medium whitespace-nowrap transition-colors',
    'aria-disabled:pointer-events-none aria-disabled:opacity-50',
  ],
  {
    variants: {
      variant: {
        primary: 'border-primary bg-primary text-primary-foreground hover:bg-primary/90',
        secondary: 'border-border bg-secondary text-secondary-foreground hover:bg-accent',
        ghost: 'border-transparent bg-transparent text-foreground hover:bg-accent',
        destructive:
          'border-destructive bg-destructive text-destructive-foreground hover:bg-destructive/90',
      },
      size: {
        sm: 'h-8 px-3 text-xs',
        md: 'h-9 px-4 text-sm',
        lg: 'h-11 px-6 text-base',
        icon: 'target-min size-9 p-0',
      },
    },
    defaultVariants: { variant: 'primary', size: 'md' },
  },
)

export interface ButtonProps
  extends Omit<ComponentPropsWithoutRef<'button'>, 'disabled'>, VariantProps<typeof button> {
  /**
   * Greys the button out and refuses its clicks, while leaving it in the tab
   * order.
   *
   * Deliberately not the native `disabled` attribute. A natively disabled
   * button cannot be focused at all, so a keyboard user tabbing through a
   * screen never learns it is there, and a screen reader never reads why it
   * cannot be used. `aria-disabled` keeps it reachable and announced.
   */
  disabled?: boolean
}

/**
 * The one button in the platform.
 *
 * It carries no per surface code. Its height is `h-9`, which is 36px for an
 * admin at a desk, 45px on a waiter's phone, and 72px on a kitchen screen,
 * because `h-9` compiles to `calc(var(--spacing) * 9)` and the surface decides
 * what `--spacing` is.
 */
export function Button({
  variant,
  size,
  disabled = false,
  className,
  type = 'button',
  onClick,
  ...rest
}: ButtonProps) {
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (disabled) {
      event.preventDefault()
      return
    }
    onClick?.(event)
  }

  return (
    <button
      type={type}
      className={cn(button({ variant, size }), className)}
      aria-disabled={disabled || undefined}
      onClick={handleClick}
      {...rest}
    />
  )
}
