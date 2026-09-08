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

/**
 * The keys in the `common` namespace, one per code the API can return.
 *
 * The conflict codes are the reason this map is worth having. A refusal that
 * arrived as the blanket word `conflict` could only ever be shown as "somebody
 * changed this first"; each of these names what actually happened, so a waiter
 * reads "a dish on this bill has not reached the table yet" and knows what to
 * do next. They come from `ConflictKind` in `api/src/domain/error.rs`, one for
 * one, and every one of them is here whether or not a screen in this slice can
 * currently provoke it, so features 12 and 15 inherit the words rather than
 * discovering the gap.
 *
 * `email_taken` is deliberately absent. `handlers/auth.rs` catches that one and
 * turns it into a field error beside the email box, so it never reaches the
 * wire as a conflict and there is nothing here to say.
 */
const MESSAGE_KEYS: Readonly<Record<string, string>> = {
  not_found: 'apiError.notFound',
  unauthenticated: 'apiError.unauthenticated',
  forbidden: 'apiError.forbidden',
  invalid: 'apiError.invalid',
  conflict: 'apiError.conflict',
  unavailable: 'apiError.unavailable',
  throttled: 'apiError.throttled',

  // The conflicts, in the order `ConflictKind` declares them.
  table_occupied: 'apiError.tableOccupied',
  visit_not_open: 'apiError.visitNotOpen',
  visit_not_closed: 'apiError.visitNotClosed',
  visit_has_open_bill: 'apiError.visitHasOpenBill',
  visit_has_unbilled_line: 'apiError.visitHasUnbilledLine',
  line_not_queued: 'apiError.lineNotQueued',
  line_not_ready: 'apiError.lineNotReady',
  line_not_served: 'apiError.lineNotServed',
  line_not_voided: 'apiError.lineNotVoided',
  round_not_ready: 'apiError.roundNotReady',
  bill_not_open: 'apiError.billNotOpen',
  line_on_closed_bill: 'apiError.lineOnClosedBill',
  bill_already_closed: 'apiError.billAlreadyClosed',
  bill_has_unserved_lines: 'apiError.billHasUnservedLines',
  bill_has_no_lines: 'apiError.billHasNoLines',
  bill_not_closed: 'apiError.billNotClosed',
  session_collision: 'apiError.sessionCollision',
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
