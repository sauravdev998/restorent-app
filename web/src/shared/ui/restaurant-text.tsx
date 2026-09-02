import type { ElementType, ReactNode } from 'react'

import { restaurantLanguage } from '@/shared/session/restaurant-settings'

import { cn } from './cn'

export interface RestaurantTextProps {
  /** The text the restaurant typed, rendered exactly as typed. */
  children: ReactNode
  /** What to render as. A `span` by default; `div` or `p` where a block is wanted. */
  as?: ElementType
  className?: string
}

/**
 * Text the restaurant typed, marked as the language it is actually in.
 *
 * A dish name, a category name, a table label, a line note, a void reason. None
 * of it is copy, none of it goes near a translation file, and none of it is ever
 * translated. It is data, and it is rendered exactly as somebody typed it.
 *
 * What this adds is the `lang` attribute. A screen reader chooses its
 * pronunciation from the nearest `lang` on the way up the tree, so a dish called
 * "मटर पनीर" sitting inside a page declared `lang="en"` gets read with English
 * phonetics, which is not an accent but noise. Marking the span as the
 * restaurant's own language is what WCAG 2.2 calls Language of Parts, and it is
 * the difference between a waiter's screen reader saying the dish and saying
 * something nobody can act on.
 *
 * The language is the restaurant's, never the reader's. The person reading may
 * have chosen English; the dish is still written in whatever the kitchen writes
 * in, and that is what has to be announced.
 *
 * `dir="auto"` alongside it, so the browser works the direction out from the
 * first strong character in the text itself. That is right rather than merely
 * convenient: a restaurant with an Arabic name on an otherwise left to right
 * page needs that one string to run the other way, and no setting on the
 * restaurant can know which of its own fields are in which script.
 */
export function RestaurantText({ children, as: Tag = 'span', className }: RestaurantTextProps) {
  return (
    <Tag lang={restaurantLanguage()} dir="auto" className={cn(className)}>
      {children}
    </Tag>
  )
}
