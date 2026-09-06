import { useQuery } from '@tanstack/react-query'
import { useRouteLoaderData } from 'react-router'

import { identityQuery, type Identity } from './identity'

/**
 * Who is signed in, as it is now rather than as it was when the route loaded.
 *
 * Every signed in screen reads the identity through this, and there are two
 * places it could come from, which is exactly the problem it solves.
 *
 * The root route's loader resolves the identity before any protected screen
 * renders, and that is what makes the gate a gate. But loader data is a
 * snapshot: React Router hands back what the loader returned and never revisits
 * it. Meanwhile `PATCH /api/me` and `PATCH /api/restaurant` both answer with a
 * fresh bundle and write it into the identity query. Screens reading the loader
 * therefore kept the old bundle, and changing your own language, or the
 * restaurant's name, did nothing anywhere on screen until a full page reload.
 *
 * So: the query is the live answer, and the loader's is the one to fall back on
 * for the first render. The loader primed that query with `ensureQueryData`, so
 * in practice they are the same object until something writes a newer one, and
 * no fetch happens here.
 *
 * Only call this inside the root route's tree. Outside it there is no loader
 * data and nobody has resolved an identity, which is the signed out case the
 * two screens outside the shell handle for themselves.
 */
export function useIdentity(): Identity {
  const loaded = useRouteLoaderData('root') as Identity
  const { data } = useQuery(identityQuery)

  return data ?? loaded
}
