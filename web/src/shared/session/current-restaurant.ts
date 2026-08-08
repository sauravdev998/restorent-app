/**
 * Which restaurant this browser is acting for.
 *
 * A placeholder. Feature 7 (accounts, restaurants, and roles) replaces the whole
 * file: the restaurant comes from the session, the server reads it from an
 * httpOnly cookie, and the browser never names it at all.
 *
 * Until then, development reads it from `?restaurant_id=` in the address bar,
 * falling back to a fixed id so the app runs with no setup. The server accepts
 * this only in development and refuses every scoped request otherwise, so this
 * cannot become a way into production data.
 */

const FALLBACK_RESTAURANT_ID = '11111111-1111-1111-1111-111111111111'

export function currentRestaurantId(): string {
  if (!import.meta.env.DEV) {
    // In production the server reads the session cookie. The browser has no
    // business naming a restaurant, and a value here would be ignored anyway.
    return ''
  }

  const fromUrl = new URLSearchParams(window.location.search).get('restaurant_id')
  return fromUrl ?? FALLBACK_RESTAURANT_ID
}
