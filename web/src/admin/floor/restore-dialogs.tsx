import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  clashingLabels,
  isSection,
  restoreSection,
  restoreTable,
  type AdminFloor,
  type ArchivedSection,
  type ArchivedTable,
} from '@/admin/api/floor'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { floorKey } from '@/shared/events/query-keys'
import { restaurantLanguage } from '@/shared/session/identity'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

export interface RestoreTableDialogProps {
  onOpenChange: (open: boolean) => void
  table: ArchivedTable
  /** For its live sections, one of which the table may go back into. */
  floor: AdminFloor
}

/**
 * Puts an archived table back, asking which section it goes into.
 *
 * It starts on the table's old section when that is still live, which is the
 * usual case, and on No section otherwise. The table keeps its label and seats.
 */
export function RestoreTableDialog({ onOpenChange, table, floor }: RestoreTableDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const sections = floor.groups.filter(isSection)
  const oldSection = table.sectionId ?? null
  const oldName = table.sectionName ?? null
  const [sectionId, setSectionId] = useState(
    table.sectionLive && oldSection !== null ? oldSection : '',
  )
  const [problem, setProblem] = useState<string | null>(null)

  const restore = useMutation({
    mutationFn: (chosen: string) => restoreTable(table.id, chosen === '' ? null : chosen),
    onSuccess: async (restored) => {
      await queryClient.invalidateQueries({ queryKey: floorKey })
      showToast({ title: t('floor.archived.tableRestored', { table: restored.label }) })
      onOpenChange(false)
    },
    onError: async (error: unknown) => {
      setProblem(apiErrorMessage(failureBody(error), common))
      await queryClient.invalidateQueries({ queryKey: floorKey })
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setProblem(null)
    restore.mutate(sectionId)
  }

  const oldSectionGone = oldName !== null && !table.sectionLive

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={t('floor.archived.restoreTableTitle', { table: table.label })}
      description={t('floor.archived.restoreTableBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={restore.isPending}>
            {restore.isPending ? t('floor.working') : t('floor.archived.restore')}
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
          label={t('floor.archived.restoreInto')}
          {...(oldSectionGone
            ? {
                hint: t('floor.archived.oldSectionGone', { section: oldName }),
              }
            : {})}
        >
          <Select
            name="sectionId"
            value={sectionId}
            onChange={(event) => {
              setSectionId(event.target.value)
            }}
          >
            <option value="">{t('floor.noSection')}</option>
            {sections.map((section) => (
              <option key={section.id} value={section.id} lang={restaurantLanguage()}>
                {section.name}
              </option>
            ))}
          </Select>
        </Field>
      </form>
    </Dialog>
  )
}

export interface RestoreSectionDialogProps {
  onOpenChange: (open: boolean) => void
  section: ArchivedSection
}

/**
 * Puts an archived section back, with the tables that were in it.
 *
 * Every archived table of the section is listed and ticked, and the admin may
 * untick any. The section and every ticked table come back together or not at
 * all; a label clash names every clashing label, so the admin can untick them.
 */
export function RestoreSectionDialog({ onOpenChange, section }: RestoreSectionDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [ticked, setTicked] = useState<ReadonlySet<string>>(
    () => new Set(section.tables.map((table) => table.id)),
  )
  const [problem, setProblem] = useState<string | null>(null)

  const restore = useMutation({
    mutationFn: (tableIds: string[]) => restoreSection(section.id, tableIds),
    onSuccess: async (restored) => {
      await queryClient.invalidateQueries({ queryKey: floorKey })
      showToast({
        title:
          restored.tables.length === 0
            ? t('floor.archived.sectionRestored', { section: restored.section.name })
            : t('floor.archived.sectionRestoredWith', {
                section: restored.section.name,
                count: restored.tables.length,
              }),
      })
      onOpenChange(false)
    },
    onError: async (error: unknown) => {
      const body = failureBody(error)
      const clashing = clashingLabels(body)

      if (clashing === null) {
        setProblem(apiErrorMessage(body, common))
      } else if (clashing.length === 0) {
        setProblem(t('floor.table.clashGone'))
      } else {
        setProblem(t('floor.archived.restoreClashes', { labels: clashing.join(', ') }))
      }

      await queryClient.invalidateQueries({ queryKey: floorKey })
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setProblem(null)
    // In the section's old order, which is the order the list shows.
    restore.mutate(section.tables.filter((table) => ticked.has(table.id)).map((table) => table.id))
  }

  function toggle(tableId: string, on: boolean): void {
    setTicked((current) => {
      const next = new Set(current)
      if (on) next.add(tableId)
      else next.delete(tableId)
      return next
    })
  }

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={t('floor.archived.restoreSectionTitle', { section: section.name })}
      description={t('floor.archived.restoreSectionBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={restore.isPending}>
            {restore.isPending ? t('floor.working') : t('floor.archived.restore')}
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

        {section.tables.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t('floor.archived.restoreSectionNone')}</p>
        ) : (
          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">
              {t('floor.archived.restoreSectionTables')}
            </legend>
            <ul className="divide-y divide-border">
              {section.tables.map((table) => (
                <li key={table.id}>
                  <label className="target-h flex cursor-pointer items-center gap-3 py-2 text-sm text-foreground">
                    <input
                      type="checkbox"
                      className="size-5 shrink-0 accent-primary"
                      checked={ticked.has(table.id)}
                      onChange={(event) => {
                        toggle(table.id, event.target.checked)
                      }}
                    />
                    <RestaurantText className="font-medium">{table.label}</RestaurantText>
                    {table.seats !== null && (
                      <span className="text-muted-foreground">
                        {t('floor.seats', { count: table.seats })}
                      </span>
                    )}
                  </label>
                </li>
              ))}
            </ul>
          </fieldset>
        )}
      </form>
    </Dialog>
  )
}
