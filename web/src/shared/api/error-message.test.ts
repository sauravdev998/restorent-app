import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'

import i18next, { changeLanguage } from '@/shared/i18n'

import { apiErrorMessage, isApiErrorBody } from './error-message'

/**
 * Turning a failed request into a sentence somebody can read.
 *
 * The real `t` is used throughout rather than a stub, deliberately. A stub
 * would prove the mapping and nothing else, and the failure that actually
 * reaches a waiter is a key this map names that no translation file has. Here a
 * missing key fails the test.
 */

const t = i18next.getFixedT('en', 'common')
const hindi = i18next.getFixedT('hi', 'common')

/**
 * The Hindi files have to be in memory before a `t` bound to Hindi can answer
 * in it, because a language with no bundle falls through to English. That
 * fallback is deliberate and is AC-3, so getting it here by accident would
 * make every Hindi assertion below pass while proving nothing.
 */
beforeAll(async () => {
  await changeLanguage('hi', 'admin')
  await changeLanguage('en', 'admin')
})

afterAll(async () => {
  await changeLanguage('en', 'admin')
})

/** Every code `api/src/presentation/error.rs` can return. */
const CODES = ['not_found', 'unauthenticated', 'forbidden', 'invalid', 'conflict', 'unavailable']

/** A failed response, shaped the way every one of them is. */
function body(error: string, message = 'an English sentence meant for a log') {
  return { error, message }
}

describe('isApiErrorBody', () => {
  it('recognises a failed response from our own API', () => {
    expect(isApiErrorBody(body('not_found'))).toBe(true)
  }) // covers: AC-3

  it('refuses a body missing either field', () => {
    expect(isApiErrorBody({ error: 'not_found' })).toBe(false)
    expect(isApiErrorBody({ message: 'no code' })).toBe(false)
  }) // covers: AC-3

  it('refuses a body whose fields are the wrong type', () => {
    // A gateway or a proxy in front of the API can return JSON that happens to
    // carry an `error` field of its own shape.
    expect(isApiErrorBody({ error: 500, message: 'upstream' })).toBe(false)
    expect(isApiErrorBody({ error: 'invalid', message: { detail: 'nested' } })).toBe(false)
  }) // covers: AC-3

  it('refuses whatever else a failed request can hand back', () => {
    // All of these are real: an empty body, an HTML error page, a timeout that
    // resolved to nothing.
    expect(isApiErrorBody(null)).toBe(false)
    expect(isApiErrorBody(undefined)).toBe(false)
    expect(isApiErrorBody('<html>502 Bad Gateway</html>')).toBe(false)
    expect(isApiErrorBody(502)).toBe(false)
    expect(isApiErrorBody({})).toBe(false)
  }) // covers: AC-3
})

describe('apiErrorMessage', () => {
  it('gives every code the API returns its own sentence', () => {
    const sentences = CODES.map((code) => apiErrorMessage(body(code), t))

    // Each one real, and each one different, so a person can tell a signed out
    // session from a role that does not allow something.
    for (const sentence of sentences) {
      expect(sentence.length).toBeGreaterThan(10)
    }
    expect(new Set(sentences).size).toBe(CODES.length)
  }) // covers: AC-1, AC-3

  it('never renders a raw key, whatever the code', () => {
    // The failure a missing translation key produces: i18next answers with the
    // key itself, and `apiError.forbidden` on a waiter's screen is machine text.
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      for (const code of [...CODES, 'some_new_code']) {
        const sentence = apiErrorMessage(body(code), t)

        expect(sentence).not.toMatch(/^apiError\./)
        expect(sentence).not.toContain('apiError')
      }
    } finally {
      noise.mockRestore()
    }
  }) // covers: AC-3

  it('never puts the API’s own English message on screen', () => {
    // The point of the split. The API is a data API and knows nothing about
    // language, so rendering its message would put one untranslated English
    // sentence in the middle of an otherwise Hindi screen.
    const message = 'no restaurant row with that id in tenant scope'

    expect(apiErrorMessage(body('not_found', message), t)).not.toContain(message)
    expect(apiErrorMessage(body('not_found', message), hindi)).not.toContain(message)
  }) // covers: AC-1

  it('never puts an unrecognised code on screen either', () => {
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      const sentence = apiErrorMessage(body('teapot_on_fire'), t)

      expect(sentence).not.toContain('teapot_on_fire')
      expect(sentence).toBe(t('apiError.unknown'))
    } finally {
      noise.mockRestore()
    }
  }) // covers: AC-1, AC-3

  it('says out loud that the API has grown a code this map has not', () => {
    // Shown as the generic sentence so nobody reads a code, and logged so
    // somebody adds the real one.
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      apiErrorMessage(body('teapot_on_fire'), t)

      expect(noise).toHaveBeenCalledOnce()
      expect(noise.mock.calls[0]?.[0]).toContain('teapot_on_fire')
    } finally {
      noise.mockRestore()
    }
  }) // covers: AC-3

  it('falls back without complaining when the body is not ours at all', () => {
    // Not a code the API grew, so there is nothing for anybody to fix and
    // nothing to log. A 502 from the load balancer arrives this way.
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      for (const value of [null, undefined, '', 'Bad Gateway', {}, []]) {
        expect(apiErrorMessage(value, t)).toBe(t('apiError.unknown'))
      }

      expect(noise).not.toHaveBeenCalled()
    } finally {
      noise.mockRestore()
    }
  }) // covers: AC-3

  it('speaks the reader’s language, not the API’s', () => {
    // The same code, twice, and only the sentence changes. This is what makes
    // the wording this side's job rather than the API's.
    for (const code of CODES) {
      const english = apiErrorMessage(body(code), t)
      const devanagari = apiErrorMessage(body(code), hindi)

      expect(devanagari).not.toBe(english)
      expect(devanagari).toMatch(/[ऀ-ॿ]/)
    }
  }) // covers: AC-2, AC-3

  it('has a Hindi sentence for the generic case too', () => {
    // The one every unrecognised code lands on, so it is the one most likely
    // to be forgotten in a translation file.
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      const sentence = apiErrorMessage(body('teapot_on_fire'), hindi)

      expect(sentence).toMatch(/[ऀ-ॿ]/)
      expect(sentence).not.toContain('apiError')
    } finally {
      noise.mockRestore()
    }
  }) // covers: AC-2, AC-3
})
