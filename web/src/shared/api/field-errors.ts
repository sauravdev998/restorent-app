import type { TFunction } from 'i18next'

import { isApiErrorBody } from './error-message'

/**
 * Reading the field level part of a failed response.
 *
 * A form gets told which box was not accepted and why, in the same closed set
 * of codes the API uses everywhere. The codes map to translation keys on this
 * side, exactly as the top level `error` code does, because the API is a data
 * API and knows nothing about what language anybody reads.
 *
 * A code this side has never heard of is dropped rather than rendered. A new
 * variant on the API must not be able to put `some_new_code` beside somebody's
 * password box.
 */

/** Every reason a field can be refused. Matches the API's closed set. */
export const FIELD_ERROR_CODES = [
  'already_taken',
  'too_short',
  'too_long',
  'invalid_format',
  'unknown_country',
  'not_in_catalogue',
  'incorrect',
  'required',
] as const

/** One of them. */
export type FieldErrorCode = (typeof FIELD_ERROR_CODES)[number]

/** Whether this is a code the interface has words for. */
export function isFieldErrorCode(value: unknown): value is FieldErrorCode {
  return typeof value === 'string' && (FIELD_ERROR_CODES as readonly string[]).includes(value)
}

/**
 * The field problems in a failed response, keyed by field name.
 *
 * An empty object means the failure was not about any particular field, which
 * is the caller's cue to say something about the request as a whole instead.
 */
export function fieldErrorsFrom(body: unknown): Record<string, FieldErrorCode> {
  if (!isApiErrorBody(body)) return {}

  const fields = (body as { fields?: unknown }).fields
  if (typeof fields !== 'object' || fields === null) return {}

  const reported: Record<string, FieldErrorCode> = {}

  for (const [field, code] of Object.entries(fields as Record<string, unknown>)) {
    if (isFieldErrorCode(code)) {
      reported[field] = code
    } else {
      console.error(`No message for the API field error code ${JSON.stringify(code)}.`)
    }
  }

  return reported
}

/**
 * The `error` prop for a `Field`, or nothing at all.
 *
 * Spread rather than passed, because `exactOptionalPropertyTypes` is on: a
 * `Field` either carries an error or does not have the prop, and
 * `error={undefined}` is a third thing TypeScript refuses. One helper, so the
 * three forms in this app cannot each get it slightly wrong.
 *
 * @param code the field's problem, or `undefined` when it has none.
 * @param t a `t` bound to the `common` namespace.
 */
export function fieldErrorProps(
  code: FieldErrorCode | undefined,
  t: TFunction,
): { error: string } | Record<string, never> {
  return code === undefined ? {} : { error: t(`fieldError.${code}`) }
}
