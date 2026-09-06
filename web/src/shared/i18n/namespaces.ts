import type { Surface } from '@/shared/surface'

/**
 * The translation files, split so a screen downloads only the words it shows.
 *
 * The split mirrors the feature folders rather than being a second organising
 * idea layered on top: `common` is what the shell and the base components say,
 * and each surface owns its own. A waiter's phone on a weak connection has no
 * reason to download the admin reporting vocabulary or the kitchen's.
 */

/** Every namespace, in every language. */
export const NAMESPACES = ['common', 'admin', 'waiter', 'kitchen'] as const

/** One of them. */
export type Namespace = (typeof NAMESPACES)[number]

/**
 * What the shell, the base components, and anything cross cutting say.
 *
 * Always loaded, in every language, on every surface. It is also the namespace
 * a bare `t('key')` reads from.
 */
export const DEFAULT_NAMESPACE: Namespace = 'common'

/**
 * Which namespaces a surface needs loaded before it can render a word.
 *
 * Called when the language changes and when the surface changes, so switching
 * language on the waiter screen never waits on the admin file.
 */
export function namespacesForSurface(surface: Surface): Namespace[] {
  return [DEFAULT_NAMESPACE, surface]
}
