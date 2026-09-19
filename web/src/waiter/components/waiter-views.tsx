import { useTranslation } from 'react-i18next'
import { NavLink } from 'react-router'

import { cn } from '@/shared/ui/cn'
import { Switch } from '@/shared/ui/switch'
import { setMineOnly, useMineOnly } from '@/waiter/mine-only'

/**
 * The switch between the waiter's two views, Floor and Orders, one tap apart,
 * with the Mine filter beside it (spec 0011, AC-1, AC-3).
 *
 * Links rather than an in page tab list, because each view is its own route:
 * a waiter who reloads Orders lands back on it, and the back button does what
 * it says. `aria-current` marks the one on screen. Each link is at least the
 * waiter surface's thumb sized target, like every button.
 *
 * The filter is one switch shared by both views, so turning it on the floor
 * leaves it on in Orders. The table screen shows the two links without it, so
 * a waiter at a table is one tap from either view.
 */
export function WaiterViews({ withMine = true }: { withMine?: boolean }) {
  const { t } = useTranslation('waiter')
  const mineOnly = useMineOnly()

  return (
    <div className="flex flex-wrap items-end justify-between gap-3">
      <nav aria-label={t('views.label')}>
        <ul className="border-b-line flex gap-2 border-border">
          <li>
            <ViewLink to="/waiter">{t('views.floor')}</ViewLink>
          </li>
          <li>
            <ViewLink to="/waiter/orders">{t('views.orders')}</ViewLink>
          </li>
        </ul>
      </nav>

      {withMine && (
        <div className="flex items-center gap-2">
          <span aria-hidden="true" className="text-sm text-muted-foreground">
            {t('views.mine')}
          </span>
          <Switch checked={mineOnly} onCheckedChange={setMineOnly} label={t('views.mineLabel')} />
        </div>
      )}
    </div>
  )
}

function ViewLink({ to, children }: { to: string; children: string }) {
  return (
    <NavLink
      to={to}
      end
      className={({ isActive }) =>
        cn(
          'target-h -mb-px inline-flex items-center border-b-4 px-4 text-base font-medium',
          isActive
            ? 'border-primary text-foreground'
            : 'border-transparent text-muted-foreground hover:text-foreground',
        )
      }
    >
      {children}
    </NavLink>
  )
}
