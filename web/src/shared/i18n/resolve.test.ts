import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import type { Identity } from '@/shared/session/identity'
import { SURFACES, type Surface } from '@/shared/surface'

import { FALLBACK_LANGUAGE } from './catalogue'
import {
  LANGUAGE_STORAGE_KEY,
  readStoredLanguage,
  resolveLanguage,
  resolveSignedInLanguage,
  resolveSignedOutLanguage,
  writeStoredLanguage,
} from './resolve'

/**
 * Which language a screen is drawn in.
 *
 * Two resolvers, deliberately not one chain, and the tests are split the same
 * way. The signed in one reads the person and the restaurant and must never
 * touch browser storage; the signed out one reads nothing but storage. Every
 * test below sets both sides explicitly, because the failure this feature
 * exists to prevent is a screen following the wrong one of them.
 */

/**
 * An identity bundle, of the shape the API really returns.
 *
 * Built here rather than mocked, because the signed in resolver now takes the
 * bundle as an argument. There is nothing left to stub: what it reads is what it
 * is handed, which is the whole improvement over the placeholder it replaced.
 */
function identity(restaurantLanguage: string, staffLanguage: string | null): Identity {
  return {
    staff: {
      id: '00000000-0000-7000-8000-000000000001',
      displayName: 'Ada Owner',
      email: 'ada@example.test',
      role: 'admin',
      language: staffLanguage,
    },
    restaurant: {
      id: '00000000-0000-7000-8000-000000000002',
      name: 'The Test Kitchen',
      address: null,
      countryCode: 'IN',
      currencyCode: 'INR',
      currencyDecimals: 2,
      timezone: 'Asia/Kolkata',
      defaultLanguage: restaurantLanguage,
      formattingLocale: 'en-IN',
    },
  }
}

/** Replaces `window.localStorage` with one that throws on every touch. */
function withRefusedStorage<T>(body: () => T): T {
  const original = Object.getOwnPropertyDescriptor(window, 'localStorage')

  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    get() {
      throw new Error('The user has blocked site data for this origin')
    },
  })

  try {
    return body()
  } finally {
    if (original === undefined) {
      Reflect.deleteProperty(window, 'localStorage')
    } else {
      Object.defineProperty(window, 'localStorage', original)
    }
  }
}

/** Makes the browser report the languages a phone might actually be set to. */
function withBrowserLanguages<T>(languages: string[], body: () => T): T {
  Object.defineProperty(window.navigator, 'languages', {
    configurable: true,
    get: () => languages,
  })

  try {
    return body()
  } finally {
    Reflect.deleteProperty(window.navigator, 'languages')
  }
}

beforeEach(() => {
  window.localStorage.clear()
})

afterEach(() => {
  window.localStorage.clear()
})

describe('the remembered choice', () => {
  it('is stored under the key the rest of the world inspects', () => {
    // Written down rather than derived. Support instructions and the verify
    // steps both name `localStorage.language`, so renaming it is a decision.
    expect(LANGUAGE_STORAGE_KEY).toBe('language')
  }) // covers: AC-7

  it('round trips through storage', () => {
    writeStoredLanguage('hi')

    expect(readStoredLanguage()).toBe('hi')
    expect(window.localStorage.getItem('language')).toBe('hi')
  }) // covers: AC-7

  it('reads as nothing when storage refuses to answer', () => {
    // A browser configured to block site data throws rather than returning
    // null, and some private modes do too. A bare read would take the app down
    // on boot.
    withRefusedStorage(() => {
      expect(readStoredLanguage()).toBeNull()
    })
  }) // covers: AC-7

  it('lets the switch go ahead when storage refuses to keep it', () => {
    // Not remembering a choice is a small loss. Refusing the switch because it
    // cannot be remembered is a screen somebody cannot read.
    withRefusedStorage(() => {
      expect(() => {
        writeStoredLanguage('hi')
      }).not.toThrow()
    })
  }) // covers: AC-7, AC-9
})

describe('resolveSignedOutLanguage', () => {
  it('opens a device nobody has touched in English', () => {
    // English is the one language every screen is guaranteed to have.
    expect(resolveSignedOutLanguage()).toBe(FALLBACK_LANGUAGE)
    expect(resolveSignedOutLanguage()).toBe('en')
  }) // covers: AC-7

  it('opens in the language somebody chose on this device', () => {
    writeStoredLanguage('hi')

    expect(resolveSignedOutLanguage()).toBe('hi')
  }) // covers: AC-7

  it('ignores what the browser reports its own language to be', () => {
    // The step the checklist could not automate. A phone set to Hindi that
    // nobody at this restaurant configured would otherwise open the sign in
    // screen in Hindi, and the person who has to fix that is the one who
    // cannot read the screen telling them how.
    withBrowserLanguages(['hi-IN', 'hi'], () => {
      expect(window.navigator.languages).toEqual(['hi-IN', 'hi'])
      expect(resolveSignedOutLanguage()).toBe('en')
    })
  }) // covers: AC-7

  it('prefers an explicit choice over what the browser reports', () => {
    writeStoredLanguage('hi')

    withBrowserLanguages(['en-GB', 'en'], () => {
      expect(resolveSignedOutLanguage()).toBe('hi')
    })
  }) // covers: AC-7

  it('falls back when the remembered code is no longer offered', () => {
    // A code left by an older build, or one whose language has since been
    // removed. The files behind it are gone, so falling back is the only thing
    // that renders.
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, 'xx')

    expect(resolveSignedOutLanguage()).toBe('en')
  }) // covers: AC-6, AC-7

  it('falls back when the remembered code is a formatting locale', () => {
    // Two lists, and only one of them has translation files behind it.
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, 'en-US')

    expect(resolveSignedOutLanguage()).toBe('en')
  }) // covers: AC-7, AC-12
})

describe('resolveSignedInLanguage', () => {
  it('follows the person’s own setting on the surfaces they own', () => {
    expect(resolveSignedInLanguage(identity('en', 'hi'), 'waiter')).toBe('hi')
    expect(resolveSignedInLanguage(identity('en', 'hi'), 'admin')).toBe('hi')
  }) // covers: AC-6

  it('leaves the kitchen on the restaurant’s language even for a chef who has one', () => {
    // The row a single shared resolver gets wrong, and the reason there are
    // two. The kitchen screen is a shared appliance read across a shift
    // handover, and a screen that changes language under somebody who has
    // learned to read it at a glance is worse than one they cannot personalise.
    expect(resolveSignedInLanguage(identity('en', 'hi'), 'kitchen')).toBe('en')
  }) // covers: AC-6

  it('sends everybody to the restaurant’s language when nobody has set their own', () => {
    // What a newly created account has, and what most accounts keep.
    for (const surface of SURFACES) {
      expect(resolveSignedInLanguage(identity('hi', null), surface)).toBe('hi')
    }
  }) // covers: AC-6

  it('ignores a personal code that is no longer offered', () => {
    expect(resolveSignedInLanguage(identity('hi', 'xx'), 'waiter')).toBe('hi')
  }) // covers: AC-6

  it('falls back to English when the restaurant’s own column is no longer offered', () => {
    // A column can outlive a catalogue entry.
    for (const surface of SURFACES) {
      expect(resolveSignedInLanguage(identity('xx', null), surface)).toBe(FALLBACK_LANGUAGE)
    }
  }) // covers: AC-6

  it('prefers a valid personal code even when the restaurant’s is broken', () => {
    expect(resolveSignedInLanguage(identity('xx', 'hi'), 'waiter')).toBe('hi')
    // And the kitchen, which skips the personal setting, still lands
    // somewhere readable rather than on the broken code.
    expect(resolveSignedInLanguage(identity('xx', 'hi'), 'kitchen')).toBe(FALLBACK_LANGUAGE)
  }) // covers: AC-6

  it('never lets a code left on a shared phone beat the signed in person', () => {
    // A stale code from the previous shift on a borrowed handset. Storage is
    // not in this chain at any point, and this is the test that says so.
    writeStoredLanguage('hi')

    for (const surface of SURFACES) {
      expect(resolveSignedInLanguage(identity('en', null), surface)).toBe('en')
    }

    // Untouched by the read, so the next sign in screen still opens in it.
    expect(readStoredLanguage()).toBe('hi')
  }) // covers: AC-6

  it('answers even when storage cannot be reached at all', () => {
    // Proof by construction that the signed in path does not read storage: it
    // still resolves in a browser where every read throws.
    withRefusedStorage(() => {
      expect(resolveSignedInLanguage(identity('hi', null), 'waiter')).toBe('hi')
    })
  }) // covers: AC-6
})

describe('resolveLanguage', () => {
  it('hands a signed in screen to the signed in resolver', () => {
    const signedIn = identity('hi', 'en')
    const surfaces: Surface[] = [...SURFACES]

    for (const surface of surfaces) {
      expect(resolveLanguage(signedIn, surface)).toBe(resolveSignedInLanguage(signedIn, surface))
    }
  }) // covers: AC-6

  it('hands a signed out screen to the signed out resolver', () => {
    // The state the app boots into: the identity has not come back yet, and the
    // sign in screen has to be readable before it does.
    writeStoredLanguage('hi')

    for (const surface of SURFACES) {
      expect(resolveLanguage(null, surface)).toBe('hi')
    }
  }) // covers: AC-6, AC-7

  it('keeps the kitchen on the restaurant’s language through the front door too', () => {
    writeStoredLanguage('en')

    const signedIn = identity('hi', 'en')
    expect(resolveLanguage(signedIn, 'kitchen')).toBe('hi')
    expect(resolveLanguage(signedIn, 'waiter')).toBe('en')
  }) // covers: AC-6
})
