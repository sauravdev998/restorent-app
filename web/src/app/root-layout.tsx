import { useQueryClient } from '@tanstack/react-query'
import { Suspense, useCallback, useEffect } from 'react'
import { Outlet, useLoaderData, useLocation, useNavigate } from 'react-router'

import { useLiveEvents } from '@/shared/events/use-live-events'
import { useDocumentLanguage } from '@/shared/i18n/use-document-language'
import { useIdentityLanguage } from '@/shared/i18n/use-identity-language'
import type { Identity } from '@/shared/session/identity'
import { signedOut } from '@/shared/session/signed-out'
import { surfaceForPath } from '@/shared/surface'
import { Skeleton } from '@/shared/ui/skeleton'
import { SurfaceShell } from '@/shared/ui/surface-shell'

/**
 * The shell every signed in screen sits inside, and the one owner of four
 * global things.
 *
 * **The live stream.** Held open once here for the whole application, not once
 * per screen.
 *
 * **The density.** `data-surface` goes on the document element rather than on a
 * wrapping div, because dialogs, toasts, and tooltips render into a portal
 * attached to `document.body`, which sits outside the React tree. A wrapper div
 * would leave every overlay at admin density in the kitchen, which is exactly
 * the screen where being unreadable hurts most.
 *
 * **The document's language.** `lang`, `dir`, and the tab title, for the same
 * reason: they live outside the React tree and nothing else updates them.
 *
 * **What happens when the session ends.** One path, whether an ordinary request
 * found out or the live stream did.
 *
 * All four are set here and nowhere else. If each `SurfaceShell` set them on
 * mount, the attributes would outlive the component that set them: walking from
 * the kitchen back to `/` would leave the whole document stuck at kitchen
 * density with nothing left to clear it.
 *
 * Nobody reaches this component signed out. The route's loader resolves the
 * identity first and redirects if there is none, so a protected screen never
 * renders for even a frame on its way to the sign in screen.
 */
export function RootLayout() {
  const identity = useLoaderData<Identity>()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const { pathname } = useLocation()
  const surface = surfaceForPath(pathname)

  // Stable, because the stream's effect depends on it and a fresh function
  // every render would tear the stream down and rebuild it on every render.
  const onSignedOut = useCallback(() => {
    void navigate(signedOut(queryClient, window.location), { replace: true })
  }, [navigate, queryClient])

  const live = useLiveEvents(onSignedOut)

  useDocumentLanguage()
  useIdentityLanguage(identity, surface)

  useEffect(() => {
    document.documentElement.dataset['surface'] = surface
  }, [surface])

  return (
    <SurfaceShell stream={live.status} surface={surface} identity={identity}>
      {/* Walking onto a surface fetches that surface's words. The boundary is
          inside the shell and not around it, so the header, the navigation, and
          the account menu stay on screen throughout: `common` is always in
          hand, and only the content column is waiting. */}
      <Suspense fallback={<Skeleton className="h-32 w-full" />}>
        <Outlet context={live} />
      </Suspense>
    </SurfaceShell>
  )
}
