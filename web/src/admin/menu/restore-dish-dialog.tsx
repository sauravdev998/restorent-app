import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { restoreDish, type AdminMenu, type ArchivedDish } from '@/admin/api/menu'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { restaurantLanguage } from '@/shared/session/identity'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

export interface RestoreDishDialogProps {
  onOpenChange: (open: boolean) => void
  dish: ArchivedDish
  /** For its live categories, one of which the dish goes back into. */
  menu: AdminMenu
}

/**
 * Puts an archived dish back, asking which live category it goes into.
 *
 * It starts on the dish's old category when that is still live, which is the
 * usual case: a seasonal dish coming back to where it always was. When the old
 * category has been removed too, it starts on the first live one and says so.
 * The dish keeps everything else, its price and its availability included.
 */
export function RestoreDishDialog({ onOpenChange, dish, menu }: RestoreDishDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [categoryId, setCategoryId] = useState(
    dish.categoryLive ? dish.categoryId : (menu.categories[0]?.id ?? ''),
  )
  const [problem, setProblem] = useState<string | null>(null)

  const restore = useMutation({
    mutationFn: () => restoreDish(dish.id, categoryId),
    onSuccess: async (restored) => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      const category = menu.categories.find((each) => each.id === restored.categoryId)
      showToast({
        title: t('menu.archived.dishRestored', {
          dish: restored.name,
          category: category?.name ?? '',
        }),
      })
      onOpenChange(false)
    },
    onError: async (error: unknown) => {
      setProblem(apiErrorMessage(failureBody(error), common))
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setProblem(null)
    restore.mutate()
  }

  const noCategory = menu.categories.length === 0

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={t('menu.archived.restoreDishTitle', { dish: dish.name })}
      description={t('menu.archived.restoreDishBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={restore.isPending || noCategory}>
            {restore.isPending ? t('menu.working') : t('menu.archived.restore')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            {t('menu.cancel')}
          </Button>
        </>
      }
    >
      <form id={formId} className="flex flex-col gap-4" onSubmit={submit} noValidate>
        {problem !== null && (
          <p role="alert" className="text-sm font-medium text-status-late">
            {problem}
          </p>
        )}

        {noCategory ? (
          <p className="text-sm text-muted-foreground">{t('menu.archived.noLiveCategory')}</p>
        ) : (
          <Field
            label={t('menu.archived.restoreInto')}
            {...(dish.categoryLive
              ? {}
              : { hint: t('menu.archived.oldCategoryGone', { category: dish.categoryName }) })}
          >
            <Select
              name="categoryId"
              value={categoryId}
              onChange={(event) => {
                setCategoryId(event.target.value)
              }}
            >
              {menu.categories.map((category) => (
                <option key={category.id} value={category.id} lang={restaurantLanguage()}>
                  {category.name}
                </option>
              ))}
            </Select>
          </Field>
        )}
      </form>
    </Dialog>
  )
}
