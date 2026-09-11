import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { createDish, type AdminMenu, type DishForm } from '@/admin/api/menu'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { DIETS } from '@/shared/api/menu'
import { parseDecimalInput } from '@/shared/format'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

export interface DishDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The menu the dialog works in: its categories and its currency. */
  menu: AdminMenu
  /** Which category the form starts on, when the admin came from one. */
  categoryId?: string
}

/**
 * The dish form, in a dialog over the menu.
 *
 * A dialog rather than a page of its own, so the admin never loses their place
 * in a long menu to add one dish to the middle of it.
 *
 * **Every rule is the API's.** The form sends what was typed, with the price box
 * turned into a plain decimal where it could be, and puts each refusal beside
 * the box it concerns, translated from its field code. Nothing is checked twice
 * with two chances to disagree.
 *
 * **The result is not written into the cache.** On success the menus are
 * invalidated and read again, which is what puts the new dish on the admin's
 * own screen at once rather than a second later when the event arrives.
 */
export function DishDialog({ open, onOpenChange, menu, categoryId }: DishDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const firstCategory = menu.categories[0]?.id ?? ''
  const [form, setForm] = useState<DishForm>(() => ({
    categoryId: categoryId ?? firstCategory,
    name: '',
    description: '',
    price: '',
    diet: 'veg',
  }))
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: (submitted: DishForm) => createDish(submitted),
    onSuccess: async (dish) => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      const category = menu.categories.find((each) => each.id === dish.categoryId)
      showToast({
        title: t('menu.dish.added', { dish: dish.name, category: category?.name ?? '' }),
      })
      onOpenChange(false)
    },
    onError: (error: unknown) => {
      const body = failureBody(error)
      const reported = fieldErrorsFrom(body)

      if (Object.keys(reported).length > 0) {
        setFields(reported)
        setProblem(null)
        return
      }

      // Not about a box: the category went away, or the request never landed.
      // Said inside the dialog, where the admin is looking, and the menu is
      // read again so the category list behind it is true.
      setFields({})
      setProblem(apiErrorMessage(body, common))
      void queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
  })

  function update<K extends keyof DishForm>(key: K, value: DishForm[K]): void {
    setForm((current) => ({ ...current, [key]: value }))
  }

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setFields({})
    setProblem(null)

    save.mutate({
      ...form,
      // A price that does not read as a number is still sent as typed, so the
      // API names what is wrong with it rather than this form guessing.
      price: parseDecimalInput(form.price) ?? form.price.trim(),
    })
  }

  const priceHint =
    menu.currencyDecimals === 0
      ? t('menu.dish.priceHintWhole', { currency: menu.currencyCode })
      : t('menu.dish.priceHint', { currency: menu.currencyCode, count: menu.currencyDecimals })

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={t('menu.dish.addTitle')}
      description={t('menu.dish.addBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending ? t('menu.dish.adding') : t('menu.dish.add')}
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

        <Field
          label={t('menu.dish.category')}
          required
          {...fieldErrorProps(fields['categoryId'], common)}
        >
          <Select
            name="categoryId"
            value={form.categoryId}
            onChange={(event) => {
              update('categoryId', event.target.value)
            }}
          >
            {menu.categories.map((category) => (
              <option key={category.id} value={category.id}>
                {category.name}
              </option>
            ))}
          </Select>
        </Field>

        <Field
          label={t('menu.dish.name')}
          hint={t('menu.dish.nameHint')}
          required
          {...fieldErrorProps(fields['name'], common)}
        >
          <Input
            name="name"
            autoComplete="off"
            value={form.name}
            onChange={(event) => {
              update('name', event.target.value)
            }}
          />
        </Field>

        <Field
          label={t('menu.dish.description')}
          hint={t('menu.dish.descriptionHint')}
          {...fieldErrorProps(fields['description'], common)}
        >
          <Input
            name="description"
            autoComplete="off"
            value={form.description}
            onChange={(event) => {
              update('description', event.target.value)
            }}
          />
        </Field>

        <div className="grid gap-4 sm:grid-cols-2">
          <Field
            label={t('menu.dish.price')}
            hint={priceHint}
            required
            {...fieldErrorProps(fields['price'], common)}
          >
            <Input
              name="price"
              inputMode="decimal"
              autoComplete="off"
              className="tabular"
              value={form.price}
              onChange={(event) => {
                update('price', event.target.value)
              }}
            />
          </Field>

          <Field label={t('menu.dish.diet')} required {...fieldErrorProps(fields['diet'], common)}>
            <Select
              name="diet"
              value={form.diet}
              onChange={(event) => {
                const chosen = DIETS.find((diet) => diet === event.target.value)
                if (chosen) update('diet', chosen)
              }}
            >
              {DIETS.map((diet) => (
                <option key={diet} value={diet}>
                  {common(`diet.${diet}`)}
                </option>
              ))}
            </Select>
          </Field>
        </div>
      </form>
    </Dialog>
  )
}
