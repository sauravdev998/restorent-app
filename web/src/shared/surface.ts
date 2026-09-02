/**
 * Which of the three surfaces a screen belongs to.
 *
 * The admin, the waiter, and the kitchen barely share a screen between them
 * even though they share a codebase, and three separate things read this: the
 * density on the document element, which translation namespace is loaded, and
 * whether a personal language setting is consulted at all.
 *
 * It is derived from the route group rather than stored, so it cannot drift out
 * of step with what is actually on screen.
 */

/** The three surfaces, and what anything unrecognised falls back to. */
export const SURFACES = ['admin', 'waiter', 'kitchen'] as const

/** One of the three. */
export type Surface = (typeof SURFACES)[number]

/**
 * What a path outside the three route groups counts as.
 *
 * Admin, because the screens that sit outside them (the system status page, a
 * not found page) are read at a desk rather than on a phone or across a
 * kitchen.
 */
export const DEFAULT_SURFACE: Surface = 'admin'

/** Which surface a pathname belongs to. */
export function surfaceForPath(pathname: string): Surface {
  const first = pathname.split('/')[1] ?? ''
  return SURFACES.find((surface) => surface === first) ?? DEFAULT_SURFACE
}

/**
 * Whether this surface follows the restaurant rather than the person.
 *
 * The kitchen screen is a shared appliance. Several chefs read it across a shift
 * handover and none of them signed into it personally, so it stays in the
 * restaurant's language whoever last touched it. A screen that changes language
 * under somebody who has learned to read it at a glance is worse than one they
 * cannot personalise.
 */
export function followsRestaurantLanguage(surface: Surface): boolean {
  return surface === 'kitchen'
}
