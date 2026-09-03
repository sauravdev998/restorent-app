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
 * `credentials: 'include'` is what carries the session: it lives in an httpOnly
 * cookie the browser attaches on its own. Same origin in both development
 * (through the Vite proxy) and production (through CloudFront), so the cookie
 * simply works.
 */
export const api = createClient<paths>({
  baseUrl: '/',
  credentials: 'include',
})

/**
 * Where a `401` on any request is handled, exactly once.
 *
 * Every request in the app goes through this client, so this is the one place
 * that has to notice a session ending. Handling it per call site would mean
 * every future screen remembering to, and the one that forgot would leave
 * somebody looking at a screen that quietly stopped working.
 *
 * The handler is registered from `main.tsx` rather than imported here, because
 * what happens next is a navigation and this module has no router.
 */
type SignedOutHandler = () => void

let onSignedOut: SignedOutHandler | null = null

/**
 * The requests whose `401` is an answer rather than a session ending.
 *
 * `GET /api/me` answers `401` for every signed out visitor, including the one
 * sitting on the sign in screen, and sign in itself answers `401` for a wrong
 * password. Treating either as "you have been signed out" would bounce somebody
 * off the screen they are trying to sign in on.
 */
const EXPECTS_401: readonly string[] = ['/api/me', '/api/auth/sign-in', '/api/auth/register']

/** Registers what to do when a request finds the session gone. */
export function handleSignedOut(handler: SignedOutHandler): void {
  onSignedOut = handler
}

api.use({
  onResponse({ response }) {
    if (response.status !== 401) return undefined

    const path = new URL(response.url, window.location.origin).pathname
    if (EXPECTS_401.includes(path)) return undefined

    onSignedOut?.()
    return undefined
  },
})
