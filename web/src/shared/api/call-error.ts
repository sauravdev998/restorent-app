/**
 * What a failed request throws, so a screen can show the right sentence.
 *
 * Every query and mutation in the app throws this rather than the raw body,
 * because TanStack Query wants an `Error` and the body is what
 * `apiErrorMessage` and `fieldErrorsFrom` read. The body travels inside it
 * untouched.
 *
 * Lives in `shared/` because all three surfaces throw it. It used to live in
 * the waiter's folder, which had the kitchen importing from the waiter's.
 */
export class ApiCallError extends Error {
  /** The failed response body, for `apiErrorMessage` to turn into words. */
  readonly body: unknown

  constructor(body: unknown) {
    super('The request was refused.')
    this.name = 'ApiCallError'
    this.body = body
  }
}

/**
 * The response body a failure carried, whatever was thrown.
 *
 * An `ApiCallError` hands over the body the API sent; anything else (a network
 * failure, a bug) is passed through, and `apiErrorMessage` turns that into its
 * generic sentence rather than a raw one.
 */
export function failureBody(error: unknown): unknown {
  return error instanceof ApiCallError ? error.body : error
}

/** The `error` code a failure carried, or `null` when it carried none. */
export function failureCode(error: unknown): string | null {
  const body = failureBody(error)
  if (typeof body !== 'object' || body === null) return null

  const code = (body as Record<string, unknown>)['error']
  return typeof code === 'string' ? code : null
}
