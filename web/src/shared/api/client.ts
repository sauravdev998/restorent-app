import createClient from 'openapi-fetch'

import type { paths } from './schema'

/**
 * The typed API client.
 *
 * Every path, parameter, and response shape comes from `schema.d.ts`, which is
 * generated from the Rust handlers. Rename a field in Rust and this stops
 * compiling, which is the entire reason the seam exists. Do not hand write a
 * fetch call to `/api` anywhere else.
 *
 * `credentials: 'include'` is here for feature 7: the session travels in an
 * httpOnly cookie. Same origin in both development (through the Vite proxy) and
 * production (through CloudFront), so the cookie simply works.
 */
export const api = createClient<paths>({
  baseUrl: '/',
  credentials: 'include',
})
