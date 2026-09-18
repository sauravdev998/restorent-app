import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  adminFloorQuery,
  createSection,
  isSection,
  renameSection,
  type LiveSection,
} from '@/admin/api/floor'
import { failureBody, failureCode } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { floorKey } from '@/shared/events/query-keys'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { showToast } from '@/shared/ui/toast-store'

export interface SectionDialogProps {
  onOpenChange: (open: boolean) => void
  /** The section to rename, or nothing to add a new one. */
  section?: LiveSection
}

/**
 * Adds a section, or renames one, in a dialog over the floor.
 *
 * A rename carries the version the dialog opened with. If somebody changed the
 * section since, the save is refused with `section_changed`; the dialog says
 * so, reads the floor again, and puts the current name in the box with the
 * current version behind it, so the admin decides on the real value.
 */
export function SectionDialog({ onOpenChange, section }: SectionDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [name, setName] = useState(section?.name ?? '')
  const [version, setVersion] = useState(section?.version ?? 0)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: (submitted: string) =>
      section ? renameSection(section.id, submitted, version) : createSection(submitted),
    onSuccess: async (saved) => {
      await queryClient.invalidateQueries({ queryKey: floorKey })
      showToast({
        title: section
          ? t('floor.section.renamed', { section: saved.name })
          : t('floor.section.added', { section: saved.name }),
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

      // Stale: show what the section is now, and save against that.
      if (section && failureCode(error) === 'section_changed') {
        const floor = await queryClient.fetchQuery({ ...adminFloorQuery, staleTime: 0 })
        const current = floor.groups.filter(isSection).find((each) => each.id === section.id)
        if (current) {
          setName(current.name)
          setVersion(current.version)
        }
        return
      }

      void queryClient.invalidateQueries({ queryKey: floorKey })
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
      open
      onOpenChange={onOpenChange}
      title={section ? t('floor.section.renameTitle') : t('floor.section.addTitle')}
      description={section ? t('floor.section.renameBody') : t('floor.section.addBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending
              ? t('floor.saving')
              : section
                ? t('floor.section.rename')
                : t('floor.section.add')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            {t('floor.cancel')}
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
          label={t('floor.section.name')}
          hint={t('floor.section.nameHint')}
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
