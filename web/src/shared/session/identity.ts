import { queryOptions, type QueryClient } from '@tanstack/react-query'

import { api } from '@/shared/api/client'
import type { components } from '@/shared/api/schema'
import {
  FALLBACK_FORMATTING_LOCALE,
  FALLBACK_LANGUAGE,
  isKnownLanguage,
} from '@/shared/i18n/catalogue'

/**
 * Who is signed in, where they work, and how this browser finds out.
 *
 * One query, one cache entry, one shape. Register, sign in, `GET /api/me`,
 * `PATCH /api/me`, and `PATCH /api/restaurant` all return the same bundle, so
 * every one of them writes into this entry rather than each screen holding its
 * own idea of who is signed in.
 *
 * The session itself is not here and cannot be: it lives in an httpOnly cookie
 * the browser attaches on its own and no script can read. This module holds
 * only what the server said about it.
 *
 * This file replaces the two placeholders spec 0005 was built against,
 * `current-restaurant.ts` and `restaurant-settings.ts`. Nothing in the browser
 * names a restaurant any more.
 */

/** Everything the browser knows about who it is talking for. */
export type Identity = components['schemas']['IdentityBundle']

/** Who is signed in. */
export type StaffIdentity = Identity['staff']

/** Where they work, and the settings every screen formats through. */
export type RestaurantIdentity = Identity['restaurant']

/** What a member of staff is allowed to be. */
export type Role = StaffIdentity['role']

/**
 * The one cache key the identity lives under.
 *
 * Exported because signing out has to clear exactly this entry, and a second
 * copy of the key spelled slightly differently is how a signed out browser ends
 * up still showing somebody's name.
 */
export const IDENTITY_KEY = ['session', 'identity'] as const

/**
 * The last identity the server sent, readable without a hook.
 *
 * The formatting layer is called from ordinary functions and not only from
 * components, so it needs a synchronous answer. This is that answer, kept in
 * step by the writers below rather than being a second source of truth anybody
 * can set.
 *
 * Read carefully what this is and is not. It is a copy of settings the server
 * already returned, and nothing decides whether anybody is signed in by reading
 * it: every request resolves its session against the database with no cache
 * anywhere, and a revoked session is refused on the very next request whatever
 * this happens to hold.
 */
let snapshot: Identity | null = null

/** The four values every formatter on a screen is built from. */
export interface RestaurantFormatting {
  /** How money, numbers, dates, and times are written here. */
  formattingLocale: string
  /** The restaurant's own IANA timezone. */
  timezone: string
  /** What it charges in, for anything that is not on a bill. */
  currencyCode: string
  /** How many decimal places that currency uses. */
  currencyDecimals: number
}

/**
 * What a signed out screen formats with.
 *
 * The sign in and registration screens show no restaurant data at all, so none
 * of this is ever seen on a real figure. It exists so those two screens render
 * without every formatter in the app growing a null check.
 *
 * The timezone is deliberately not the device's. A formatter that happens to
 * agree with the machine it runs on hides the entire class of bug this project
 * cares about, which is a timestamp written in the wrong place's clock.
 */
const SIGNED_OUT_FORMATTING: RestaurantFormatting = {
  formattingLocale: FALLBACK_FORMATTING_LOCALE,
  timezone: 'UTC',
  currencyCode: 'USD',
  currencyDecimals: 2,
}

/** What the server last said, or `null` if it has not said anything yet. */
export function currentIdentity(): Identity | null {
  return snapshot
}

/** How this screen writes money, numbers, dates, and times. */
export function restaurantFormatting(): RestaurantFormatting {
  const restaurant = snapshot?.restaurant
  if (!restaurant) return SIGNED_OUT_FORMATTING

  return {
    formattingLocale: restaurant.formattingLocale,
    timezone: restaurant.timezone,
    currencyCode: restaurant.currencyCode,
    currencyDecimals: restaurant.currencyDecimals,
  }
}

/**
 * The restaurant's own language, guarded against a code no longer offered.
 *
 * A column can outlive a catalogue entry. When it does, the files behind that
 * code are gone, so falling back is the only thing that renders.
 */
export function restaurantLanguage(): string {
  const code = snapshot?.restaurant.defaultLanguage
  return isKnownLanguage(code) ? code : FALLBACK_LANGUAGE
}

/**
 * The signed in identity, or `null` when nobody is.
 *
 * A `401` is a real answer here rather than a failure: it means nobody is
 * signed in, which is the ordinary state of the sign in screen itself. Throwing
 * would drop every signed out visitor into an error boundary.
 *
 * Never refetched on a timer. The server is the authority on whether the
 * session is still alive and says so on every request the app makes anyway;
 * polling this would add a request a minute per screen to learn something the
 * next real request would have said.
 */
export const identityQuery = queryOptions({
  queryKey: IDENTITY_KEY,
  queryFn: async (): Promise<Identity | null> => {
    const { data, error, response } = await api.GET('/api/me')

    if (data) {
      snapshot = data
      return data
    }

    if (response.status === 401) {
      snapshot = null
      return null
    }

    throw new Error(
      error ? `Could not read the session (${response.status}).` : 'Could not reach the API.',
    )
  },
  staleTime: Infinity,
  retry: false,
})

/**
 * Puts a freshly returned bundle into the cache.
 *
 * Called by every endpoint that returns one, so signing in, editing your own
 * language, and an admin editing the restaurant all land in the same place and
 * every screen re draws from it at once.
 */
export function rememberIdentity(queryClient: QueryClient, identity: Identity): void {
  snapshot = identity
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, identity)
}

/**
 * Forgets who was signed in.
 *
 * Set to `null` rather than removed. Removing would leave the query pending on
 * the next render, so a screen would show a loading state on its way to the
 * sign in screen; `null` is the answer, and it is immediate.
 */
export function forgetIdentity(queryClient: QueryClient): void {
  snapshot = null
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, null)
}
