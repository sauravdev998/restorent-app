import { describe, expect, it, vi } from 'vitest'

import { FIELD_ERROR_CODES, fieldErrorsFrom, isFieldErrorCode } from './field-errors'

/**
 * Reading the field level part of a failed response.
 *
 * The codes are a closed set the API and this side both hold. What matters here
 * is the two edges: a body with no field problem must not look like one, and a
 * code this side has never heard of must not reach a screen.
 */

describe('fieldErrorsFrom', () => {
  it('reads each named field’s reason', () => {
    const reported = fieldErrorsFrom({
      error: 'invalid',
      message: 'for logs only',
      fields: { email: 'already_taken', password: 'too_short' },
    })

    expect(reported).toEqual({ email: 'already_taken', password: 'too_short' })
  }) // covers: AC-2

  it('reports nothing for a failure that is not about any field', () => {
    // The caller's cue to say something about the request as a whole instead,
    // which is what the sign in screen does with a refused pair.
    expect(fieldErrorsFrom({ error: 'unauthenticated', message: 'no' })).toEqual({})
    expect(fieldErrorsFrom({ error: 'invalid', message: 'no', fields: null })).toEqual({})
    expect(fieldErrorsFrom(undefined)).toEqual({})
    expect(fieldErrorsFrom('not a body at all')).toEqual({})
  }) // covers: AC-2

  it('drops a code this side has no words for, and says so in the console', () => {
    // A new variant on the API must not be able to put `some_new_code` beside
    // somebody's password box.
    const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    const fields = fieldErrorsFrom({
      error: 'invalid',
      message: 'no',
      fields: { email: 'already_taken', mystery: 'a_code_from_the_future' },
    })

    expect(fields).toEqual({ email: 'already_taken' })
    expect(reported).toHaveBeenCalledOnce()

    reported.mockRestore()
  })
})

describe('the closed set of codes', () => {
  it('holds exactly what the API can send, and nothing else', () => {
    // Written out rather than derived, because a rule that generated this list
    // would generate the same mistake on both sides. These are the strings
    // `api/src/domain/error.rs` returns.
    expect([...FIELD_ERROR_CODES]).toEqual([
      'already_taken',
      'too_short',
      'too_long',
      'invalid_format',
      'unknown_country',
      'not_in_catalogue',
      'incorrect',
      'required',
      'not_a_number',
      'negative',
      'too_large',
      'too_many_decimals',
    ])

    for (const code of FIELD_ERROR_CODES) {
      expect(isFieldErrorCode(code)).toBe(true)
    }

    expect(isFieldErrorCode('nearly_correct')).toBe(false)
    expect(isFieldErrorCode(7)).toBe(false)
    expect(isFieldErrorCode(null)).toBe(false)
  }) // covers: AC-2
})
