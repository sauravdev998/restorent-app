import type { Identity } from '@/shared/session/identity'
import { followsRestaurantLanguage, type Surface } from '@/shared/surface'

import { FALLBACK_LANGUAGE, isKnownLanguage } from './catalogue'

/**
 * Which language a screen is drawn in.
 *
 * There are two resolvers here and deliberately not one chain, because they read
 * different places and only one of them is allowed near browser storage.
 *
 * Signed out, there is no person and no restaurant, so the only thing left is
 * what somebody explicitly chose on this device. Signed in, there is a person
 * and a restaurant, and the device is irrelevant: a code left on a shared phone
 * by the previous shift must never beat the signed in person's own setting.
 *
 * Writing is not reading. A signed in switch writes to both places, so the next
 * sign in screen on that phone opens in the language the last person used, and
 * the signed in resolver still never looks at it.
 */

/**
 * Where an explicit choice made while signed out is remembered.
 *
 * A language code and nothing else. On a shared phone the next waiter inherits
 * the last explicit choice, which is intended: a house phone in a Hindi speaking
 * restaurant should stay in Hindi.
 */
export const LANGUAGE_STORAGE_KEY = 'language'

/**
 * Reads the remembered choice, if there is a readable one.
 *
 * Storage throws rather than returning null in a browser configured to block
 * site data, and in some private modes, so this can never be a bare read.
 */
export function readStoredLanguage(): string | null {
  try {
    return window.localStorage.getItem(LANGUAGE_STORAGE_KEY)
  } catch {
    return null
  }
}

/** Remembers an explicit choice on this device, if storage will have it. */
export function writeStoredLanguage(code: string): void {
  try {
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, code)
  } catch {
    // A browser that refuses storage is not a reason to refuse the switch. The
    // language still changes; it simply is not remembered past this tab.
  }
}

/**
 * The language for a device with nobody signed in.
 *
 * An explicit choice on this device, else English.
 *
 * The browser's own `navigator.languages` is deliberately not consulted. A
 * phone set to a language nobody at this restaurant reads would otherwise open
 * the sign in screen in it, and the person who has to fix that is the one who
 * cannot read the screen telling them how. English is the one language every
 * screen is guaranteed to have, so a device nobody has configured opens in it
 * and anybody can then choose.
 */
export function resolveSignedOutLanguage(): string {
  const stored = readStoredLanguage()
  return isKnownLanguage(stored) ? stored : FALLBACK_LANGUAGE
}

/**
 * The language for a signed in person on a given surface.
 *
 * Their own setting, else the restaurant's, else English. On the kitchen surface
 * the personal setting is skipped entirely, even when a chef who has one is
 * signed in: that screen is a shared appliance read across a shift handover.
 *
 * `localStorage` is not in this chain at any point. Both values come from the
 * identity bundle, which came from the session, which came from the database.
 */
export function resolveSignedInLanguage(identity: Identity, surface: Surface): string {
  if (!followsRestaurantLanguage(surface)) {
    const personal = identity.staff.language
    if (isKnownLanguage(personal)) return personal
  }

  const restaurant = identity.restaurant.defaultLanguage
  return isKnownLanguage(restaurant) ? restaurant : FALLBACK_LANGUAGE
}

/**
 * The language a screen on this surface should currently be drawn in.
 *
 * `null` means nobody is signed in, which is the state the app boots into: the
 * identity has not come back yet, and the sign in screen has to be readable
 * before it does.
 */
export function resolveLanguage(identity: Identity | null, surface: Surface): string {
  return identity ? resolveSignedInLanguage(identity, surface) : resolveSignedOutLanguage()
}
