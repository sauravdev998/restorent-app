import { Suspense, useEffect } from 'react'
import { Outlet, useLocation } from 'react-router'

import { useLiveEvents } from '@/shared/events/use-live-events'
import { useDocumentLanguage } from '@/shared/i18n/use-document-language'
import { surfaceForPath } from '@/shared/surface'
import { Skeleton } from '@/shared/ui/skeleton'
import { SurfaceShell } from '@/shared/ui/surface-shell'

/**
 * The shell every screen sits inside, and the one owner of three global things.
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
 * All three are set here and nowhere else. If each `SurfaceShell` set them on
 * mount, the attributes would outlive the component that set them: walking from
 * the kitchen back to `/` would leave the whole document stuck at kitchen
 * density with nothing left to clear it.
 */
export function RootLayout() {
  const live = useLiveEvents()
  const { pathname } = useLocation()
  const surface = surfaceForPath(pathname)

  useDocumentLanguage()

  useEffect(() => {
    document.documentElement.dataset['surface'] = surface
  }, [surface])

  return (
    <SurfaceShell stream={live.status} surface={surface}>
      {/* Walking onto a surface fetches that surface's words. The boundary is
          inside the shell and not around it, so the header, the navigation, and
          the language switcher stay on screen throughout: `common` is always in
          hand, and only the content column is waiting. */}
      <Suspense fallback={<Skeleton className="h-32 w-full" />}>
        <Outlet context={live} />
      </Suspense>
    </SurfaceShell>
  )
}
