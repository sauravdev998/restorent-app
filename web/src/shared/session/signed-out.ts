import type { QueryClient } from '@tanstack/react-query'

import { forgetIdentity } from './identity'

/**
 * What happens when the server says nobody is signed in.
 *
 * One path, used by three things that all discover it differently: an ordinary
 * request that came back `401`, the live event stream failing fatally, and a
 * screen that loaded while already signed out. Three separate handlings would
 * drift, and the one that drifted would be the one that leaves a waiter looking
 * at a screen that no longer refreshes.
 *
 * It does two things. It forgets the cached identity, so nothing on screen goes
 * on showing a name or a restaurant that is no longer signed in. And it sends
 * the person to the sign in screen carrying the path they were on, so signing
 * back in returns them to the ticket they were halfway through rather than to
 * the top of the app.
 */

/** Where somebody signs in. */
export const SIGN_IN_PATH = '/sign-in'

/** Where somebody registers a new restaurant. */
export const REGISTER_PATH = '/register'

/** The query parameter carrying where to go back to. */
export const NEXT_PARAM = 'next'

/** Screens anybody may open without being signed in. */
const PUBLIC_PATHS: readonly string[] = [SIGN_IN_PATH, REGISTER_PATH]

/** Whether this path is one a signed out visitor may see. */
export function isPublicPath(pathname: string): boolean {
  return PUBLIC_PATHS.includes(pathname)
}

/**
 * The sign in address that returns somebody to where they were.
 *
 * The public screens are excluded from the return path deliberately: being sent
 * back to the sign in screen after signing in is a loop, and it is exactly what
 * a naive "remember the last path" does.
 */
export function signInPathFor(pathname: string, search = ''): string {
  if (isPublicPath(pathname)) return SIGN_IN_PATH

  const next = `${pathname}${search}`
  return `${SIGN_IN_PATH}?${NEXT_PARAM}=${encodeURIComponent(next)}`
}

/**
 * Where to go after signing in.
 *
 * A `next` that is not a path on this site is ignored rather than followed. It
 * arrives from the address bar, so it is somebody's input: without this check a
 * link to `/sign-in?next=https://elsewhere.example` would send a person who
 * signed in on our own screen straight off it, with our name on the page they
 * left from.
 */
export function returnPathFrom(search: string, fallback: string): string {
  const next = new URLSearchParams(search).get(NEXT_PARAM)

  if (next === null) return fallback
  // A single leading slash, and never two: `//elsewhere.example` is a protocol
  // relative address that a browser reads as another site.
  if (!next.startsWith('/') || next.startsWith('//')) return fallback

  return next
}

/**
 * Which surface a role lands on, and the only screens it may open.
 *
 * The role decides where signing in lands somebody, and a person who opens a
 * surface their role does not hold is sent to the one it does. That is a
 * convenience rather than a control: the server refuses the request whatever
 * the browser shows.
 */
export const LANDING: Readonly<Record<string, string>> = {
  admin: '/admin',
  waiter: '/waiter',
  chef: '/kitchen',
}

/** Where somebody of this role belongs. */
export function landingFor(role: string): string {
  return LANDING[role] ?? '/'
}

/**
 * Forgets who was signed in, and says where to send them.
 *
 * Deliberately does not navigate. Navigation belongs to whoever holds the
 * router, and a module that reached for a global one would be untestable and
 * would fire during a render.
 */
export function signedOut(queryClient: QueryClient, from: Location | URL): string {
  forgetIdentity(queryClient)
  return signInPathFor(from.pathname, from.search)
}
