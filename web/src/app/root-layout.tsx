import { useTranslation } from 'react-i18next'
import { NavLink, Outlet } from 'react-router'

import { useLiveEvents } from '@/shared/events/use-live-events'

const NAV = [
  { to: '/admin', key: 'nav.admin' },
  { to: '/waiter', key: 'nav.waiter' },
  { to: '/kitchen', key: 'nav.kitchen' },
] as const

/**
 * The shell every screen sits inside.
 *
 * It is also where the live stream is held open, once, for the whole
 * application. One stream per browser, not one per screen.
 */
export function RootLayout() {
  const { t } = useTranslation()
  const live = useLiveEvents()

  return (
    <div className="min-h-screen">
      <header className="border-b border-slate-200 dark:border-slate-800">
        <nav className="mx-auto flex max-w-4xl items-center gap-6 px-4 py-3" aria-label="Main">
          <NavLink to="/" className="font-semibold">
            {t('app.name')}
          </NavLink>
          <ul className="flex gap-4">
            {NAV.map((item) => (
              <li key={item.to}>
                <NavLink
                  to={item.to}
                  className={({ isActive }) =>
                    isActive ? 'underline underline-offset-4' : 'text-slate-500 hover:underline'
                  }
                >
                  {t(item.key)}
                </NavLink>
              </li>
            ))}
          </ul>
          <span
            className="ml-auto text-sm text-slate-500"
            // Announced politely so a screen reader mentions a dropped
            // connection without interrupting whatever is being read.
            aria-live="polite"
            data-testid="stream-status"
          >
            {t(`stream.${live.status}`)}
          </span>
        </nav>
      </header>

      <main className="mx-auto max-w-4xl px-4 py-8">
        <Outlet context={live} />
      </main>
    </div>
  )
}
