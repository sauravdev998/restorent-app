import { useTranslation } from 'react-i18next'

import type { Diet } from '@/shared/api/menu'

import { cn } from './cn'

export interface DietMarkProps {
  diet: Diet
  /** `sm` beside a line of body text, `md` where the dish name is the main thing. */
  size?: 'sm' | 'md'
  className?: string
}

/** The colour class for each mark. The colour repeats the shape; it never replaces it. */
const COLOUR: Readonly<Record<Diet, string>> = {
  veg: 'text-diet-veg',
  non_veg: 'text-diet-non-veg',
  egg: 'text-diet-egg',
}

const SIZE = {
  sm: 'size-4',
  md: 'size-5',
} as const

/**
 * The square veg, non veg, or egg mark a customer in India reads before
 * anything else on a menu.
 *
 * All three sit inside the same square outline, and each fills it with its own
 * shape: a circle for veg, a triangle for non veg, an oval for egg. **The shape
 * is the meaning, never the colour alone.** The colour repeats it for a quick
 * glance, and that is the one place the design system spends colour on
 * something other than order status, deliberately and in writing, in
 * `docs/design.md`.
 *
 * So it holds up everywhere the colour does not: under forced colours every
 * mark is the system's own text colour and the shapes still differ; on paper
 * they are ink; and for a screen reader the mark is an image named in the
 * reader's own language.
 *
 * Sized by spacing steps, so it grows with the surface's density like every
 * icon: small at a desk, large on a kitchen wall.
 */
export function DietMark({ diet, size = 'sm', className }: DietMarkProps) {
  const { t } = useTranslation()

  return (
    <svg
      role="img"
      aria-label={t(`diet.${diet}`)}
      viewBox="0 0 16 16"
      className={cn(
        'shrink-0',
        SIZE[size],
        COLOUR[diet],
        'forced-colors:text-[CanvasText]',
        className,
      )}
    >
      <rect
        x="1"
        y="1"
        width="14"
        height="14"
        rx="1.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      {diet === 'veg' && <circle cx="8" cy="8" r="3.5" fill="currentColor" />}
      {diet === 'non_veg' && <path d="M8 4.25 L11.75 11.25 H4.25 Z" fill="currentColor" />}
      {diet === 'egg' && <ellipse cx="8" cy="8" rx="2.75" ry="3.75" fill="currentColor" />}
    </svg>
  )
}
