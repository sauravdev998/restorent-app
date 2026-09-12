import { useMutation, useQueryClient } from '@tanstack/react-query'
import { ArchiveRestore } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { restoreCategory, type AdminMenu, type ArchivedDish } from '@/admin/api/menu'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { formatTimestamp } from '@/shared/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { DietMark } from '@/shared/ui/diet-mark'
import { Icon } from '@/shared/ui/icon'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { showToast } from '@/shared/ui/toast-store'

export interface ArchivedSectionProps {
  menu: AdminMenu
  /** Opens the restore dialog for a dish, which asks which category it goes into. */
  onRestoreDish: (dish: ArchivedDish) => void
}

/**
 * Everything taken off the menu, and the way back for each.
 *
 * A removed dish or category is off every other screen, but not gone: every
 * bill that ever charged for it still reads exactly as it did, and it comes
 * back from here with everything it had. That is why removing asks one plain
 * question rather than making the admin type anything to confirm.
 *
 * A category comes back with one tap, at the end of the list. A dish asks
 * which live category it goes into, because its old one may have gone too.
 */
export function ArchivedSection({ menu, onRestoreDish }: ArchivedSectionProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const [restoring, setRestoring] = useState<string | null>(null)

  const restore = useMutation({
    mutationFn: (categoryId: string) => restoreCategory(categoryId),
    onMutate: (categoryId: string) => {
      setRestoring(categoryId)
    },
    onSuccess: async (category) => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      showToast({ title: t('menu.archived.categoryRestored', { category: category.name }) })
    },
    onError: async (error: unknown) => {
      // Most often `name_taken`: a live category has had its name meanwhile.
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
    onSettled: () => {
      setRestoring(null)
    },
  })

  const { categories, dishes } = menu.archived
  const empty = categories.length === 0 && dishes.length === 0

  return (
    <section aria-labelledby="archived-heading" className="space-y-3">
      <div className="max-w-prose">
        <h2 id="archived-heading" className="text-lg font-semibold text-foreground">
          {t('menu.archived.title')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">{t('menu.archived.intro')}</p>
      </div>

      {empty ? (
        <p className="border-line rounded-md border-dashed border-border p-4 text-sm text-muted-foreground">
          {t('menu.archived.empty')}
        </p>
      ) : (
        <Card className="space-y-4">
          {categories.length > 0 && (
            <div className="space-y-1">
              <h3 className="text-sm font-medium text-muted-foreground">
                {t('menu.archived.categories')}
              </h3>
              <ul className="divide-y divide-border">
                {categories.map((category) => (
                  <li
                    key={category.id}
                    className="flex flex-wrap items-center gap-x-4 gap-y-1 py-2"
                  >
                    <RestaurantText className="min-w-0 flex-1 truncate font-medium text-card-foreground">
                      {category.name}
                    </RestaurantText>
                    <span className="text-sm text-muted-foreground">
                      {t('menu.archived.removedAt', {
                        when: formatTimestamp(category.archivedAt),
                      })}
                    </span>
                    <Button
                      variant="secondary"
                      size="sm"
                      disabled={restoring === category.id}
                      aria-label={t('menu.archived.restoreNamed', { name: category.name })}
                      onClick={() => {
                        restore.mutate(category.id)
                      }}
                    >
                      <Icon icon={ArchiveRestore} size="sm" />
                      {restoring === category.id ? t('menu.working') : t('menu.archived.restore')}
                    </Button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {dishes.length > 0 && (
            <div className="space-y-1">
              <h3 className="text-sm font-medium text-muted-foreground">
                {t('menu.archived.dishes')}
              </h3>
              <ul className="divide-y divide-border">
                {dishes.map((dish) => (
                  <li key={dish.id} className="flex flex-wrap items-center gap-x-4 gap-y-1 py-2">
                    <DietMark diet={dish.diet} />
                    <div className="min-w-0 flex-1">
                      <RestaurantText as="p" className="truncate font-medium text-card-foreground">
                        {dish.name}
                      </RestaurantText>
                      <p className="truncate text-xs text-muted-foreground">
                        {dish.categoryLive
                          ? t('menu.archived.from', { category: dish.categoryName })
                          : t('menu.archived.fromRemoved', { category: dish.categoryName })}
                      </p>
                    </div>
                    <span className="text-sm text-muted-foreground">
                      {t('menu.archived.removedAt', { when: formatTimestamp(dish.archivedAt) })}
                    </span>
                    <Button
                      variant="secondary"
                      size="sm"
                      aria-label={t('menu.archived.restoreNamed', { name: dish.name })}
                      onClick={() => {
                        onRestoreDish(dish)
                      }}
                    >
                      <Icon icon={ArchiveRestore} size="sm" />
                      {t('menu.archived.restore')}
                    </Button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Card>
      )}
    </section>
  )
}
