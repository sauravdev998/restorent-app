import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  adminMenuQuery,
  createDish,
  editDish,
  type AdminMenu,
  type DishForm,
} from '@/admin/api/menu'
import { failureBody, failureCode } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { DIETS, type Dish } from '@/shared/api/menu'
import { parseDecimalInput } from '@/shared/format'
import { restaurantLanguage } from '@/shared/session/identity'
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
  /** The dish to edit, or nothing to add a new one. */
  dish?: Dish
}

/** What the form holds for a dish that already exists. */
function formFor(dish: Dish, decimals: number): DishForm {
  return {
    categoryId: dish.categoryId,
    name: dish.name,
    description: dish.description ?? '',
    price: plainPrice(dish.price, decimals),
    diet: dish.diet,
  }
}

/**
 * A stored price as a person would type it.
 *
 * The column holds four places, so `320` comes back as `320.0000`. The box
 * shows it with the currency's own places instead, and the trimmed digits are
 * always zeros, because the API refuses a price with more places than that.
 */
function plainPrice(price: string, decimals: number): string {
  const [whole = price, fraction = ''] = price.split('.')
  if (decimals === 0) return whole
  return `${whole}.${fraction.padEnd(decimals, '0').slice(0, decimals)}`
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
export function DishDialog({ open, onOpenChange, menu, categoryId, dish }: DishDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const firstCategory = menu.categories[0]?.id ?? ''
  const [form, setForm] = useState<DishForm>(() =>
    dish
      ? formFor(dish, menu.currencyDecimals)
      : {
          categoryId: categoryId ?? firstCategory,
          name: '',
          description: '',
          price: '',
          diet: 'veg',
        },
  )
  const [version, setVersion] = useState(dish?.version ?? 0)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: (submitted: DishForm) =>
      dish ? editDish(dish.id, submitted, version) : createDish(submitted),
    onSuccess: async (saved) => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      const category = menu.categories.find((each) => each.id === saved.categoryId)
      showToast({
        title: dish
          ? t('menu.dish.saved', { dish: saved.name })
          : t('menu.dish.added', { dish: saved.name, category: category?.name ?? '' }),
      })
      onOpenChange(false)
    },
    onError: async (error: unknown) => {
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

      // Stale, most often because the kitchen switched the dish off since the
      // form opened: show what the dish is now, and save against that.
      if (dish && failureCode(error) === 'dish_changed') {
        const fresh = await queryClient.fetchQuery({ ...adminMenuQuery, staleTime: 0 })
        const current = fresh.categories
          .flatMap((category) => category.dishes)
          .find((each) => each.id === dish.id)
        if (current) {
          setForm(formFor(current, fresh.currencyDecimals))
          setVersion(current.version)
        }
        return
      }

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
      title={dish ? t('menu.dish.editTitle') : t('menu.dish.addTitle')}
      description={dish ? t('menu.dish.editBody') : t('menu.dish.addBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending ? t('menu.saving') : dish ? t('menu.dish.save') : t('menu.dish.add')}
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
          {...(dish ? { hint: t('menu.dish.moveHint') } : {})}
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
              <option key={category.id} value={category.id} lang={restaurantLanguage()}>
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
