import { useQuery } from '@tanstack/react-query'
import { BookOpenText, FolderPlus, Pencil, Plus } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { adminMenuQuery, type AdminCategory, type AdminMenu } from '@/admin/api/menu'
import { CategoryDialog } from '@/admin/menu/category-dialog'
import { DishDialog } from '@/admin/menu/dish-dialog'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import type { Dish } from '@/shared/api/menu'
import { useDishAvailability, type DishAvailability } from '@/shared/api/use-dish-availability'
import { formatMoney } from '@/shared/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { DietMark } from '@/shared/ui/diet-mark'
import { EmptyState } from '@/shared/ui/empty-state'
import { Icon } from '@/shared/ui/icon'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'
import { Switch } from '@/shared/ui/switch'

/** Which dialog is open, and what it was opened from. */
type Opened =
  | { kind: 'dish'; categoryId?: string; dish?: Dish }
  | { kind: 'category'; category?: AdminCategory }
  | null

/**
 * The admin's menu: every category and dish, laid out the way the printed menu
 * is, with every change made in place.
 *
 * One page, categories as stacked sections, because that is how an owner
 * thinks about a menu and how a waiter reads one. Adding or editing a dish
 * opens a dialog over it rather than a page of its own, so a long menu never
 * loses the admin's place.
 *
 * **Every change is live the moment it saves.** There is no draft. The page
 * says so in its introduction, because an owner reworking the menu at eight in
 * the evening is changing the waiters' screens mid service, one save at a time.
 *
 * **Nothing is written into the cache ahead of the server.** A write that
 * succeeds invalidates `['dish']`, which refreshes this screen at once and the
 * ordering menu with it; everybody else's screen learns of it from the event.
 */
export function AdminMenuScreen() {
  const { t } = useTranslation(['admin', 'common'])
  const { t: common } = useTranslation()

  const menu = useQuery(adminMenuQuery)
  const availability = useDishAvailability()
  const [opened, setOpened] = useState<Opened>(null)

  const heading = (
    <div className="max-w-prose">
      <h1 className="text-2xl font-semibold text-foreground">{t('menu.title')}</h1>
      <p className="mt-1 text-sm text-muted-foreground">{t('menu.intro')}</p>
    </div>
  )

  if (menu.isPending) {
    return (
      <div className="space-y-6">
        {heading}
        <Skeleton className="h-40 w-full" label={common('loading.label')} />
        <Skeleton className="h-40 w-full" />
      </div>
    )
  }

  if (menu.isError) {
    return (
      <div className="space-y-6">
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

  const { categories } = menu.data
  const dishes = categories.flatMap((category) => category.dishes)
  const off = dishes.filter((dish) => !dish.available).length

  return (
    <div className="space-y-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="space-y-2">
          {heading}
          {categories.length > 0 && (
            <p className="text-sm text-muted-foreground">
              {[
                t('menu.categoryCount', { count: categories.length }),
                t('menu.dishCount', { count: dishes.length }),
                ...(off > 0 ? [t('menu.offCount', { count: off })] : []),
              ].join(' · ')}
            </p>
          )}
        </div>

        {categories.length > 0 && (
          <div className="flex flex-wrap gap-2">
            <Button
              variant="secondary"
              onClick={() => {
                setOpened({ kind: 'category' })
              }}
            >
              <Icon icon={FolderPlus} size="sm" />
              {t('menu.addCategory')}
            </Button>
            <Button
              onClick={() => {
                setOpened({ kind: 'dish' })
              }}
            >
              <Icon icon={Plus} size="sm" />
              {t('menu.addDish')}
            </Button>
          </div>
        )}
      </header>

      {categories.length === 0 ? (
        <EmptyState
          icon={BookOpenText}
          title={t('menu.emptyTitle')}
          description={t('menu.emptyBody')}
          action={
            <Button
              onClick={() => {
                setOpened({ kind: 'category' })
              }}
            >
              <Icon icon={FolderPlus} size="sm" />
              {t('menu.addFirstCategory')}
            </Button>
          }
        />
      ) : (
        <ol className="space-y-6">
          {categories.map((category) => (
            <li key={category.id}>
              <CategorySection
                category={category}
                menu={menu.data}
                availability={availability}
                onAddDish={() => {
                  setOpened({ kind: 'dish', categoryId: category.id })
                }}
                onRename={() => {
                  setOpened({ kind: 'category', category })
                }}
                onEditDish={(dish) => {
                  setOpened({ kind: 'dish', dish })
                }}
              />
            </li>
          ))}
        </ol>
      )}

      {opened?.kind === 'dish' && (
        <DishDialog
          open
          onOpenChange={(open) => {
            if (!open) setOpened(null)
          }}
          menu={menu.data}
          {...(opened.categoryId === undefined ? {} : { categoryId: opened.categoryId })}
          {...(opened.dish === undefined ? {} : { dish: opened.dish })}
        />
      )}

      {opened?.kind === 'category' && (
        <CategoryDialog
          open
          onOpenChange={(open) => {
            if (!open) setOpened(null)
          }}
          {...(opened.category === undefined ? {} : { category: opened.category })}
        />
      )}
    </div>
  )
}

interface CategorySectionProps {
  category: AdminCategory
  menu: AdminMenu
  availability: DishAvailability
  onAddDish: () => void
  onRename: () => void
  onEditDish: (dish: Dish) => void
}

/** One category: its heading, its actions, and its dishes in printed order. */
function CategorySection({
  category,
  menu,
  availability,
  onAddDish,
  onRename,
  onEditDish,
}: CategorySectionProps) {
  const { t } = useTranslation('admin')
  const headingId = `category-${category.id}`

  return (
    <Card as="section" aria-labelledby={headingId} className="space-y-3">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 items-baseline gap-3">
          <h2 id={headingId} className="truncate text-lg font-semibold text-card-foreground">
            <RestaurantText>{category.name}</RestaurantText>
          </h2>
          <span className="text-sm text-muted-foreground">
            {t('menu.dishCount', { count: category.dishes.length })}
          </span>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            variant="ghost"
            size="sm"
            aria-label={t('menu.renameCategoryNamed', { category: category.name })}
            onClick={onRename}
          >
            <Icon icon={Pencil} size="sm" />
            {t('menu.renameCategory')}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            aria-label={t('menu.addDishTo', { category: category.name })}
            onClick={onAddDish}
          >
            <Icon icon={Plus} size="sm" />
            {t('menu.addDishHere')}
          </Button>
        </div>
      </header>

      {category.dishes.length === 0 ? (
        <p className="border-line rounded-md border-dashed border-border p-4 text-sm text-muted-foreground">
          {t('menu.categoryEmpty')}
        </p>
      ) : (
        <ul className="divide-y divide-border">
          {category.dishes.map((dish) => (
            <DishRow
              key={dish.id}
              dish={dish}
              menu={menu}
              availability={availability}
              onEdit={() => {
                onEditDish(dish)
              }}
            />
          ))}
        </ul>
      )}
    </Card>
  )
}

interface DishRowProps {
  dish: Dish
  menu: AdminMenu
  availability: DishAvailability
  onEdit: () => void
}

/** One dish: what it is, what it costs, and whether the kitchen can make it. */
function DishRow({ dish, menu, availability, onEdit }: DishRowProps) {
  const { t } = useTranslation('admin')
  const available = availability.valueFor(dish.id, dish.available)

  return (
    <li className="flex flex-wrap items-center gap-x-4 gap-y-2 py-3">
      <DietMark diet={dish.diet} size="md" />

      <div className="min-w-0 flex-1">
        <p className="truncate text-base font-medium text-card-foreground">
          <RestaurantText>{dish.name}</RestaurantText>
        </p>
        {dish.description !== null && (
          <p className="truncate text-sm text-muted-foreground">
            <RestaurantText>{dish.description}</RestaurantText>
          </p>
        )}
      </div>

      <span className="tabular text-sm text-card-foreground">
        {formatMoney(dish.price, menu.currencyCode, menu.currencyDecimals)}
      </span>

      <div className="flex items-center gap-2">
        <Switch
          checked={available}
          label={t('menu.availabilityLabel', { dish: dish.name })}
          onCheckedChange={(next) => {
            availability.set(dish.id, next, dish.name)
          }}
        />
        <span
          className={
            available
              ? 'min-w-24 text-sm text-card-foreground'
              : 'min-w-24 text-sm font-medium text-muted-foreground'
          }
        >
          {available ? t('menu.available') : t('menu.off')}
        </span>
      </div>

      <Button
        variant="ghost"
        size="sm"
        aria-label={t('menu.editDishNamed', { dish: dish.name })}
        onClick={onEdit}
      >
        <Icon icon={Pencil} size="sm" />
        {t('menu.editDish')}
      </Button>
    </li>
  )
}
