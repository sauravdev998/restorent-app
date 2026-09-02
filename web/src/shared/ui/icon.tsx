import type { LucideIcon } from 'lucide-react'

import { cn } from './cn'

/** How big an icon is, in steps that ride the surface's spacing scale. */
export type IconSize = 'sm' | 'md' | 'lg'

const SIZES: Readonly<Record<IconSize, string>> = {
  sm: 'size-4',
  md: 'size-5',
  lg: 'size-6',
}

export interface IconProps {
  /** The Lucide component to draw, e.g. `Flame`. */
  icon: LucideIcon
  /**
   * What the icon means, for a screen reader.
   *
   * Leave it out and the icon is decorative: it is hidden from assistive
   * technology, because the text beside it already says the same thing and
   * hearing it twice is noise.
   */
  label?: string
  /** Defaults to `md`. */
  size?: IconSize
  className?: string
}

/**
 * The one place a Lucide icon is drawn.
 *
 * Two rules live here and both are load bearing.
 *
 * It sizes itself with a `size-*` class and never passes Lucide's own `size`
 * prop. That prop writes literal pixel `width` and `height` onto the SVG, which
 * no CSS variable can reach, so an icon set that way stays exactly as big on a
 * kitchen wall as it is on a laptop while the type around it triples. A class
 * rides `--spacing` and grows with everything else.
 *
 * Its stroke weight comes from `--icon-stroke` through a class rather than
 * through Lucide's `strokeWidth` prop, for the same reason: a presentation
 * attribute cannot hold a `var()`, but a CSS declaration can, and a CSS
 * declaration also beats the attribute Lucide writes. So a hairline glyph at a
 * desk becomes a heavy one across a hot kitchen.
 */
export function Icon({ icon: Glyph, label, size = 'md', className }: IconProps) {
  return (
    <Glyph
      className={cn(SIZES[size], 'stroke-token shrink-0', className)}
      aria-hidden={label === undefined ? true : undefined}
      role={label === undefined ? undefined : 'img'}
      aria-label={label}
      focusable="false"
    />
  )
}
