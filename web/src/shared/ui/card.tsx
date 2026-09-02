import type { ReactNode } from 'react'

import { cn } from './cn'

export interface CardProps {
  children: ReactNode
  /** Renders as `<article>` when the card is a self contained thing, e.g. a ticket. */
  as?: 'div' | 'article' | 'li' | 'section'
  className?: string
  /** Used when `as` is `section` or `article` and the card has its own heading. */
  'aria-labelledby'?: string
}

/**
 * The surface a ticket or a panel sits on.
 *
 * Its boundary is a real border rather than a shade of background, for two
 * reasons that are the same reason: a shade disappears entirely in forced
 * colours mode, and it is invisible from three metres away. The border's weight
 * comes from `--border-width`, so it is a hairline for an admin and a line you
 * can actually see in a kitchen.
 */
export function Card({ children, as: Element = 'div', className, ...rest }: CardProps) {
  return (
    <Element
      className={cn(
        'border-line rounded-lg border-border bg-card p-4 text-card-foreground',
        className,
      )}
      {...rest}
    >
      {children}
    </Element>
  )
}
