import { useTranslation } from 'react-i18next'
import { NavLink } from 'react-router'

import { cn } from '@/shared/ui/cn'

/**
 * The two kitchen screens, side by side at the top of each: the pass and the
 * menu.
 *
 * Links rather than an in page tab list, because each is its own route: a chef
 * who reloads the Menu tab lands back on it, and the browser's back button
 * does what it says. `aria-current` marks the one on screen, which is what a
 * screen reader announces for the current page in a set of links.
 *
 * Sized by the kitchen's own density like everything else, so each link is as
 * easy to hit with a gloved hand as a Done button.
 */
export function KitchenTabs() {
  const { t } = useTranslation('kitchen')

  return (
    <nav aria-label={t('tabs.label')}>
      <ul className="border-b-line flex gap-2 border-border">
        <li>
          <KitchenTab to="/kitchen">{t('tabs.pass')}</KitchenTab>
        </li>
        <li>
          <KitchenTab to="/kitchen/menu">{t('tabs.menu')}</KitchenTab>
        </li>
      </ul>
    </nav>
  )
}

function KitchenTab({ to, children }: { to: string; children: string }) {
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
