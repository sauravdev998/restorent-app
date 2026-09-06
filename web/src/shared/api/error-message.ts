import type { TFunction } from 'i18next'

/**
 * Turning a failed request into a sentence a person can read.
 *
 * Every failed response from the API carries the same two fields: a stable
 * `error` code meant to be branched on, and an English `message` meant for a
 * log. Only the code is used here.
 *
 * That split is the whole point. The API is a data API and knows nothing about
 * language, so rendering its `message` would put one untranslated English
 * sentence in the middle of an otherwise Hindi screen, and would quietly make
 * the API responsible for the product's wording. Mapping the code to a key
 * keeps the words on this side, where they are translated with everything else.
 *
 * The codes are the ones `api/src/presentation/error.rs` returns. A code this
 * map has never heard of still produces a real sentence rather than a raw code,
 * because a new variant on the API must not be able to put machine text on a
 * waiter's screen.
 */

/** The shape every failed API response has. */
export interface ApiErrorBody {
  /** The stable machine readable code. The only field rendered from. */
  error: string
  /** English, for the logs. Never shown to anybody. */
  message: string
}

/** The keys in the `common` namespace, one per code the API can return. */
const MESSAGE_KEYS: Readonly<Record<string, string>> = {
  not_found: 'apiError.notFound',
  unauthenticated: 'apiError.unauthenticated',
  forbidden: 'apiError.forbidden',
  invalid: 'apiError.invalid',
  conflict: 'apiError.conflict',
  unavailable: 'apiError.unavailable',
}

/** What anything unrecognised says. */
const FALLBACK_KEY = 'apiError.unknown'

/** Whether this is a failed response body from our own API. */
export function isApiErrorBody(value: unknown): value is ApiErrorBody {
  if (typeof value !== 'object' || value === null) return false
  const body = value as Record<string, unknown>
  return typeof body['error'] === 'string' && typeof body['message'] === 'string'
}

/**
 * The sentence to show for a failed request.
 *
 * @param body the response body, whatever came back.
 * @param t a `t` bound to the `common` namespace.
 */
export function apiErrorMessage(body: unknown, t: TFunction): string {
  if (!isApiErrorBody(body)) return t(FALLBACK_KEY)

  const key = MESSAGE_KEYS[body.error]

  if (key === undefined) {
    // A code the API has grown and this map has not. Logged so it is fixed, and
    // shown as the generic sentence so nobody reads `some_new_code` on a screen.
    console.error(`No message key for the API error code ${JSON.stringify(body.error)}.`)
    return t(FALLBACK_KEY)
  }

  return t(key)
}
