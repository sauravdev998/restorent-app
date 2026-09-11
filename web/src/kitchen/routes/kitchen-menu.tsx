import { useQuery } from '@tanstack/react-query'
import { BookOpenText } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { KitchenTabs } from '@/kitchen/components/kitchen-tabs'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { menuQuery } from '@/shared/api/menu'
import { useDishAvailability } from '@/shared/api/use-dish-availability'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { DietMark } from '@/shared/ui/diet-mark'
import { EmptyState } from '@/shared/ui/empty-state'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'
import { Switch } from '@/shared/ui/switch'

/**
 * The kitchen's Menu tab: every live dish, grouped by category, with a switch
 * per dish.
 *
 * Availability is the only menu change a chef can make, and this screen is
 * built for exactly that one act. The moment the kitchen runs out of something,
 * a chef taps its switch and every waiter's ordering screen greys the dish
 * within a second or two, with no admin in the loop.
 *
 * It reads the same ordering menu the waiter orders from, so what the chef sees
 * as off is exactly what the waiter sees greyed. No price is shown: a kitchen
 * screen has no business with one.
 *
 * Kitchen density, like the pass: the `data-surface` on the document makes the
 * switches and the words big enough to read and hit from across the kitchen,
 * with no size class of its own here.
 */
export function KitchenMenu() {
  const { t } = useTranslation(['kitchen', 'common'])
  const { t: common } = useTranslation()

  const menu = useQuery(menuQuery)
  const availability = useDishAvailability()

  const heading = (
    <header className="space-y-1">
      <h1 className="text-2xl font-semibold text-foreground">{t('menu.title')}</h1>
      <p className="text-sm text-muted-foreground">{t('menu.intro')}</p>
    </header>
  )

  if (menu.isPending) {
    return (
      <div className="space-y-4">
        <KitchenTabs />
        {heading}
        <Skeleton className="h-72 w-full" label={common('loading.label')} />
      </div>
    )
  }

  if (menu.isError) {
    return (
      <div className="space-y-4">
        <KitchenTabs />
        {heading}
        <EmptyState
          icon={BookOpenText}
          title={common('error.title')}
          description={apiErrorMessage(failureBody(menu.error), common)}
          action={
            <Button
              onClick={() => {
                void menu.refetch()
              }}
            >
              {common('error.retry')}
            </Button>
          }
        />
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <KitchenTabs />
      {heading}

      {menu.data.categories.length === 0 ? (
        <EmptyState
          icon={BookOpenText}
          title={t('menu.emptyTitle')}
          description={t('menu.emptyBody')}
        />
      ) : (
        <div className="grid gap-4 xl:grid-cols-2">
          {menu.data.categories.map((category) => {
            const headingId = `kitchen-category-${category.id}`

            return (
              <Card
                as="section"
                key={category.id}
                aria-labelledby={headingId}
                className="space-y-2"
              >
                <h2 id={headingId} className="text-xl font-semibold text-card-foreground">
                  <RestaurantText>{category.name}</RestaurantText>
                </h2>

                <ul className="divide-y divide-border">
                  {category.dishes.map((dish) => {
                    const available = availability.valueFor(dish.id, dish.available)

                    return (
                      <li key={dish.id} className="flex items-center gap-4 py-2">
                        <DietMark diet={dish.diet} />
                        <RestaurantText
                          as="p"
                          className={
                            available
                              ? 'min-w-0 truncate text-base font-medium text-card-foreground'
                              : 'min-w-0 truncate text-base font-medium text-muted-foreground line-through'
                          }
                        >
                          {dish.name}
                        </RestaurantText>

                        <div className="ms-auto flex shrink-0 items-center gap-3">
                          <span
                            className={
                              available
                                ? 'text-sm text-card-foreground'
                                : 'text-sm font-semibold text-card-foreground'
                            }
                          >
                            {available ? t('menu.on') : t('menu.off')}
                          </span>
                          <Switch
                            checked={available}
                            label={t('menu.availabilityLabel', { dish: dish.name })}
                            onCheckedChange={(next) => {
                              availability.set(dish.id, next, dish.name)
                            }}
                          />
                        </div>
                      </li>
                    )
                  })}
                </ul>
              </Card>
            )
          })}
        </div>
      )}
    </div>
  )
}
