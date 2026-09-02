import { Utensils } from 'lucide-react'
import { useEffect, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { NavLink } from 'react-router'

import type { StreamStatus } from '@/shared/events/use-live-events'
import { LanguageSwitcher } from '@/shared/i18n/language-switcher'
import { followsRestaurantLanguage, type Surface } from '@/shared/surface'

import { primeAudioUnlock } from './audio-unlock'
import { cn } from './cn'
import { ConnectionStatus } from './connection-status'
import { Icon } from './icon'
import { LiveRegion } from './live-region'
import { ToastViewport } from './toast'

const NAV = [
  { to: '/admin', key: 'nav.admin' },
  { to: '/waiter', key: 'nav.waiter' },
  { to: '/kitchen', key: 'nav.kitchen' },
] as const

export interface SurfaceShellProps {
  children: ReactNode
  /** The live stream's state, shown in the header of every surface. */
  stream: StreamStatus
  /** Which surface this is, which decides whether the language is a choice. */
  surface: Surface
  className?: string
}

/**
 * The frame every screen sits inside.
 *
 * It owns the landmarks (one header, one nav, one main), the skip link that
 * lets a keyboard user jump the navigation, and the width of the content
 * column.
 *
 * The width is the interesting part. `max-w-*` lives in Tailwind's container
 * namespace, which does not ride `--spacing`, so it would stay exactly as wide
 * on a kitchen wall as on a laptop while everything inside it tripled. The
 * shell takes the width from its own `--shell-width` token instead, which the
 * density layer sets per surface: a wide column for admin paperwork, a narrow
 * one for a phone, and the whole screen for a kitchen.
 *
 * The header is deliberately not sticky. A sticky bar is the usual way a focus
 * ring ends up hidden behind something, and no screen here needs one.
 *
 * It also mounts the two things that must exist exactly once per document: the
 * live regions every announcement goes through, and the toast viewport. And it
 * primes the audio unlock on the session's first tap or key press, because that
 * gesture is the only moment a browser will open an audio context.
 */
export function SurfaceShell({ children, stream, surface, className }: SurfaceShellProps) {
  const { t } = useTranslation()

  useEffect(() => primeAudioUnlock(), [])

  return (
    <div className={cn('flex min-h-screen flex-col bg-background', className)}>
      {/* Parked just above the viewport rather than hidden with `sr-only`.
          `not-sr-only` resets padding and margin to zero as part of undoing the
          hiding, which left the link visible but with its text jammed against
          its own edges. Sliding it into view keeps every other style intact. */}
      <a
        href="#main-content"
        className="fixed top-0 start-4 z-50 -translate-y-full rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground focus:translate-y-4 print:hidden"
      >
        {t('a11y.skipToMain')}
      </a>

      <header className="border-b-line border-border bg-card">
        <div className="shell-width flex flex-wrap items-center gap-4 px-4 py-3">
          <NavLink
            to="/"
            className="inline-flex items-center gap-2 text-base font-semibold text-foreground"
          >
            <Icon icon={Utensils} size="md" className="text-primary" />
            {t('app.name')}
          </NavLink>

          <nav aria-label={t('a11y.primaryNav')}>
            <ul className="flex flex-wrap items-center gap-1">
              {NAV.map((item) => (
                <li key={item.to}>
                  <NavLink
                    to={item.to}
                    className={({ isActive }) =>
                      cn(
                        'target-h inline-flex items-center rounded-md px-3 py-2 text-sm transition-colors',
                        isActive
                          ? 'bg-secondary font-medium text-secondary-foreground'
                          : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
                      )
                    }
                  >
                    {t(item.key)}
                  </NavLink>
                </li>
              ))}
            </ul>
          </nav>

          <div className="ms-auto flex items-center gap-4">
            <ConnectionStatus status={stream} />
            {/* Absent from the kitchen, which is a shared appliance following
                the restaurant's own language rather than whoever last walked
                past it. Absent by not being rendered, not by being disabled:
                there is nothing here for a chef to decide. */}
            {!followsRestaurantLanguage(surface) && <LanguageSwitcher surface={surface} />}
          </div>
        </div>
      </header>

      {/* `tabIndex={-1}` is what makes the skip link actually skip. Following a
          fragment link moves where the next Tab starts, but it only moves focus
          itself if the target can hold focus, and a `<main>` cannot by default.
          Without it, Safari and several screen readers leave focus on the body
          and the link does nothing for exactly the people it exists for. */}
      <main id="main-content" tabIndex={-1} className="shell-width w-full flex-1 px-4 py-8">
        {children}
      </main>

      <LiveRegion />
      <ToastViewport />
    </div>
  )
}
