import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  adminMenuQuery,
  createCategory,
  renameCategory,
  type AdminCategory,
} from '@/admin/api/menu'
import { failureBody, failureCode } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { showToast } from '@/shared/ui/toast-store'

export interface CategoryDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The category to rename, or nothing to add a new one. */
  category?: AdminCategory
}

/**
 * Adds a category, or renames one, in a dialog over the menu.
 *
 * A rename carries the version the dialog opened with. If somebody changed the
 * category since, the save is refused with `category_changed`; the dialog then
 * says so, reads the menu again, and puts the current name in the box with the
 * current version behind it, so the admin decides on the real value rather than
 * writing over it blind.
 */
export function CategoryDialog({ open, onOpenChange, category }: CategoryDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [name, setName] = useState(category?.name ?? '')
  const [version, setVersion] = useState(category?.version ?? 0)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: (submitted: string) =>
      category ? renameCategory(category.id, submitted, version) : createCategory(submitted),
    onSuccess: async (saved) => {
      await queryClient.invalidateQueries({ queryKey: ['dish'] })
      showToast({
        title: category
          ? t('menu.category.renamed', { category: saved.name })
          : t('menu.category.added', { category: saved.name }),
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

      setFields({})
      setProblem(apiErrorMessage(body, common))

      // Stale: show what the category is now, and save against that.
      if (category && failureCode(error) === 'category_changed') {
        const menu = await queryClient.fetchQuery({ ...adminMenuQuery, staleTime: 0 })
        const current = menu.categories.find((each) => each.id === category.id)
        if (current) {
          setName(current.name)
          setVersion(current.version)
        }
        return
      }

      void queryClient.invalidateQueries({ queryKey: ['dish'] })
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setFields({})
    setProblem(null)
    save.mutate(name)
  }

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={category ? t('menu.category.renameTitle') : t('menu.category.addTitle')}
      description={category ? t('menu.category.renameBody') : t('menu.category.addBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending
              ? t('menu.saving')
              : category
                ? t('menu.category.rename')
                : t('menu.category.add')}
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
          label={t('menu.category.name')}
          hint={t('menu.category.nameHint')}
          required
          {...fieldErrorProps(fields['name'], common)}
        >
          <Input
            name="name"
            autoComplete="off"
            value={name}
            onChange={(event) => {
              setName(event.target.value)
            }}
          />
        </Field>
      </form>
    </Dialog>
  )
}
