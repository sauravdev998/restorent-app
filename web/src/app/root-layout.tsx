import { useEffect } from 'react'
import { Outlet, useLocation } from 'react-router'

import { useLiveEvents } from '@/shared/events/use-live-events'
import { SurfaceShell } from '@/shared/ui/surface-shell'

/** The three surfaces, and what anything unrecognised falls back to. */
const SURFACES = ['admin', 'waiter', 'kitchen'] as const

type Surface = (typeof SURFACES)[number]

const DEFAULT_SURFACE: Surface = 'admin'

function surfaceForPath(pathname: string): Surface {
  const first = pathname.split('/')[1] ?? ''
  return SURFACES.find((surface) => surface === first) ?? DEFAULT_SURFACE
}

/**
 * The shell every screen sits inside, and the one owner of two global things.
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
 * It is set here and nowhere else. If each `SurfaceShell` set it on mount, the
 * attribute would outlive the component that set it: walking from the kitchen
 * back to `/` would leave the whole document stuck at kitchen density with
 * nothing left to clear it.
 */
export function RootLayout() {
  const live = useLiveEvents()
  const { pathname } = useLocation()

  useEffect(() => {
    document.documentElement.dataset['surface'] = surfaceForPath(pathname)
  }, [pathname])

  return (
    <SurfaceShell stream={live.status}>
      <Outlet context={live} />
    </SurfaceShell>
  )
}
