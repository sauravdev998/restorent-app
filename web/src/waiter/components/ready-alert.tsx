import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'

import { Button } from '@/shared/ui/button'
import { Icon } from '@/shared/ui/icon'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { STATUS_PRESENTATION } from '@/shared/ui/status'
import { byTable, type ReadyDish } from '@/waiter/alerts/ready'

export interface ReadyAlertProps {
  /** The ready dishes on my tables that I have not acknowledged yet. */
  dishes: readonly ReadyDish[]
  /** Stops the reminders for every dish shown. */
  onAcknowledge: () => void
}

/**
 * The ready alert, as the waiter sees it on any waiter screen (spec 0011,
 * AC-9).
 *
 * Names every table with food up and every dish on it, links straight to each
 * table, and carries the one button that stops the two minute reminder. It is
 * drawn from the same list the reminder runs on, so what is on screen and
 * what keeps chiming can never disagree.
 *
 * Sticky at the top of the content, so a waiter scrolled halfway down Orders
 * still sees it. The announcement and the chime are the hook's job, not this
 * component's: a banner that re rendered with one more dish must not chime
 * again.
 *
 * Colour is never the only signal: the ready icon, the word, and the dish
 * names all say the same thing.
 */
export function ReadyAlert({ dishes, onAcknowledge }: ReadyAlertProps) {
  const { t } = useTranslation('waiter')
  const presentation = STATUS_PRESENTATION.ready

  if (dishes.length === 0) return null

  const tables = byTable(dishes)

  return (
    <section
      aria-labelledby="ready-alert-title"
      data-tone="ready"
      className={`border-line sticky top-2 z-10 flex flex-col gap-3 rounded-lg border-current bg-card p-4 shadow-lg ${presentation.text}`}
    >
      <div className="flex items-start gap-3">
        <Icon icon={presentation.icon} size="lg" />
        <div className="min-w-0 flex-1">
          <h2 id="ready-alert-title" className="text-base font-semibold">
            {t('alert.title', { count: dishes.length })}
          </h2>
          <ul className="mt-2 space-y-2 text-sm text-card-foreground">
            {tables.map((table) => (
              <li key={table.visitId}>
                <Link
                  to={`/waiter/tables/${table.visitId}`}
                  className="font-semibold text-foreground underline underline-offset-4"
                >
                  {t('alert.table', { label: table.tableLabel })}
                </Link>
                <span className="text-muted-foreground"> · </span>
                {table.dishes.map((dish, index) => (
                  <span key={dish.lineId}>
                    {index > 0 && ', '}
                    {dish.quantity} × <RestaurantText>{dish.dishName}</RestaurantText>
                  </span>
                ))}
              </li>
            ))}
          </ul>
        </div>
      </div>

      <Button variant="secondary" onClick={onAcknowledge} className="self-end">
        {t('alert.acknowledge')}
      </Button>
    </section>
  )
}
