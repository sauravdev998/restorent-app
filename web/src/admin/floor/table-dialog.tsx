import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  adminFloorQuery,
  clashingLabels,
  createTable,
  createTableRange,
  editTable,
  isSection,
  type AdminFloor,
  type AdminTable,
} from '@/admin/api/floor'
import { failureBody, failureCode } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { floorKey } from '@/shared/events/query-keys'
import { restaurantLanguage } from '@/shared/session/identity'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

/** The most tables one range adds, the same ceiling the API holds. */
const RANGE_MAX = 50

/** The numbers a range may use, the same bounds the API holds. */
const NUMBER_MIN = 1
const NUMBER_MAX = 999

export interface TableDialogProps {
  onOpenChange: (open: boolean) => void
  /** The floor the dialog works in, for its live sections. */
  floor: AdminFloor
  /** Which group the form starts on: a section id, or `null` for none. */
  sectionId?: string | null
  /** The table to edit, with the group it is shown in, or nothing to add. */
  table?: { table: AdminTable; sectionId: string | null }
}

/** Whether the add form makes one table or a numbered range. */
const MODES = ['one', 'several'] as const

type Mode = (typeof MODES)[number]

/** What the form holds, exactly as typed. */
interface FormState {
  sectionId: string
  label: string
  seats: string
  prefix: string
  from: string
  to: string
}

/** A box that should hold a whole number: empty, a number, or not one. */
type Whole = { kind: 'empty' } | { kind: 'number'; value: number } | { kind: 'not_a_number' }

function wholeNumber(text: string): Whole {
  const trimmed = text.trim()
  if (trimmed === '') return { kind: 'empty' }
  if (!/^-?\d+$/.test(trimmed)) return { kind: 'not_a_number' }
  return { kind: 'number', value: Number(trimmed) }
}

/** A stored seat count as the box shows it. */
function seatsText(seats: number | null | undefined): string {
  return seats === null || seats === undefined ? '' : String(seats)
}

/** What one save sends: the form, and whether it is a range. */
interface Submission {
  form: FormState
  several: boolean
}

/** The labels a range would make, when the numbers make a range at all. */
function preview(form: FormState): { first: string; last: string; count: number } | null {
  const from = wholeNumber(form.from)
  const to = wholeNumber(form.to)
  if (from.kind !== 'number' || to.kind !== 'number') return null
  if (from.value < NUMBER_MIN || to.value > NUMBER_MAX || to.value < from.value) return null

  const count = to.value - from.value + 1
  if (count > RANGE_MAX) return null

  // The same rule the API applies: leading spaces go, trailing ones stay.
  const prefix = form.prefix.trimStart()
  return { first: `${prefix}${String(from.value)}`, last: `${prefix}${String(to.value)}`, count }
}

/**
 * The table form, in a dialog over the floor.
 *
 * Adding offers one table or a numbered range, such as T1 to T30 in one step.
 * Editing changes one table's label, seats, and section, and is allowed while a
 * party sits at it.
 *
 * **Every rule is the API's**, with one exception the spec names: a box that
 * should hold a whole number and holds something else is refused here, with the
 * same words, because a JSON number cannot carry `abc` to the API at all.
 *
 * **A clash in a range lists every clashing label**, from the refusal itself,
 * so a table a colleague added a second ago is named too.
 *
 * **The result is not written into the cache.** On success the floors are
 * invalidated and read again.
 */
export function TableDialog({ onOpenChange, floor, sectionId, table }: TableDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const sections = floor.groups.filter(isSection)

  const [mode, setMode] = useState<Mode>('one')
  const [form, setForm] = useState<FormState>(() => ({
    sectionId: (table ? table.sectionId : sectionId) ?? '',
    label: table?.table.label ?? '',
    seats: seatsText(table?.table.seats),
    prefix: '',
    from: '',
    to: '',
  }))
  const [version, setVersion] = useState(table?.table.version ?? 0)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)
  const [clashes, setClashes] = useState<string[] | null>(null)

  const change = (key: keyof FormState) => (value: string) => {
    setForm((current) => ({ ...current, [key]: value }))
  }

  const several = !table && mode === 'several'
  const range = preview(form)

  const save = useMutation({
    mutationFn: async ({
      form: submitted,
      several: asRange,
    }: Submission): Promise<{ label: string; count: number }> => {
      const section = submitted.sectionId === '' ? null : submitted.sectionId
      const seats = wholeNumber(submitted.seats)
      const seatsValue = seats.kind === 'number' ? seats.value : null

      if (table) {
        const saved = await editTable(
          table.table.id,
          { sectionId: section, label: submitted.label, seats: seatsValue },
          version,
        )
        return { label: saved.label, count: 1 }
      }

      if (asRange) {
        const from = wholeNumber(submitted.from)
        const to = wholeNumber(submitted.to)
        const created = await createTableRange({
          sectionId: section,
          prefix: submitted.prefix,
          from: from.kind === 'number' ? from.value : 0,
          to: to.kind === 'number' ? to.value : 0,
          seats: seatsValue,
        })
        return { label: created[0]?.label ?? '', count: created.length }
      }

      const created = await createTable({
        sectionId: section,
        label: submitted.label,
        seats: seatsValue,
      })
      return { label: created.label, count: 1 }
    },
    // Whether it was a range comes from what was sent, never from this render:
    // the callbacks run with the latest render's closure (see web/AGENTS.md).
    onSuccess: async (saved, { several: asRange }) => {
      await queryClient.invalidateQueries({ queryKey: floorKey })
      showToast({
        title: table
          ? t('floor.table.saved', { table: saved.label })
          : asRange
            ? t('floor.table.addedSeveral', { count: saved.count })
            : t('floor.table.added', { table: saved.label }),
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

      const clashing = clashingLabels(body)
      if (clashing !== null) {
        // The list itself is the message, beside the numbers that made it.
        setClashes(clashing)
        setProblem(null)
        void queryClient.invalidateQueries({ queryKey: floorKey })
        return
      }

      setProblem(apiErrorMessage(body, common))

      // Stale: show what the table is now, and save against that.
      if (table && failureCode(error) === 'table_changed') {
        const fresh = await queryClient.fetchQuery({ ...adminFloorQuery, staleTime: 0 })
        for (const group of fresh.groups) {
          const current = group.tables.find((each) => each.id === table.table.id)
          if (current) {
            setForm((was) => ({
              ...was,
              sectionId: group.id ?? '',
              label: current.label,
              seats: seatsText(current.seats),
            }))
            setVersion(current.version)
          }
        }
        return
      }

      void queryClient.invalidateQueries({ queryKey: floorKey })
    },
  })

  /** The whole number boxes this form checks itself before sending. */
  function localErrors(): Record<string, FieldErrorCode> {
    const found: Record<string, FieldErrorCode> = {}
    const boxes: (keyof FormState)[] = several ? ['from', 'to', 'seats'] : ['seats']

    for (const box of boxes) {
      const value = wholeNumber(form[box])
      if (value.kind === 'not_a_number') found[box] = 'not_a_number'
      if (value.kind === 'empty' && box !== 'seats') found[box] = 'required'
    }

    return found
  }

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setProblem(null)
    setClashes(null)

    const local = localErrors()
    setFields(local)
    if (Object.keys(local).length > 0) return

    save.mutate({ form, several })
  }

  const sectionField = (
    <Field label={t('floor.table.section')} {...(table ? { hint: t('floor.table.moveHint') } : {})}>
      <Select
        name="sectionId"
        value={form.sectionId}
        onChange={(event) => {
          change('sectionId')(event.target.value)
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
  )

  const seatsField = (
    <Field
      label={t('floor.table.seats')}
      hint={t('floor.table.seatsHint')}
      {...fieldErrorProps(fields['seats'], common)}
    >
      <Input
        name="seats"
        inputMode="numeric"
        autoComplete="off"
        value={form.seats}
        onChange={(event) => {
          change('seats')(event.target.value)
        }}
      />
    </Field>
  )

  const title = table
    ? t('floor.table.editTitle', { table: table.table.label })
    : t('floor.table.addTitle')

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={title}
      description={table ? t('floor.table.editBody') : t('floor.table.addBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending
              ? t('floor.saving')
              : table
                ? t('floor.table.save')
                : several
                  ? t('floor.table.addSeveral')
                  : t('floor.table.add')}
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
        {!table && (
          <div role="group" aria-label={t('floor.table.modeLabel')} className="flex gap-2">
            {MODES.map((option) => (
              <Button
                key={option}
                type="button"
                size="sm"
                variant={mode === option ? 'primary' : 'secondary'}
                aria-pressed={mode === option}
                className="flex-1"
                onClick={() => {
                  setMode(option)
                  setFields({})
                  setClashes(null)
                  setProblem(null)
                }}
              >
                {option === 'one' ? t('floor.table.modeOne') : t('floor.table.modeSeveral')}
              </Button>
            ))}
          </div>
        )}

        {problem !== null && (
          <p role="alert" className="text-sm font-medium text-status-late">
            {problem}
          </p>
        )}

        {clashes !== null && (
          <p role="alert" className="text-sm font-medium text-status-late">
            {clashes.length === 0
              ? t('floor.table.clashGone')
              : t('floor.table.clashes', { labels: clashes.join(', ') })}
          </p>
        )}

        {several ? (
          <>
            <Field
              label={t('floor.table.prefix')}
              hint={t('floor.table.prefixHint')}
              {...fieldErrorProps(fields['prefix'], common)}
            >
              <Input
                name="prefix"
                autoComplete="off"
                value={form.prefix}
                onChange={(event) => {
                  change('prefix')(event.target.value)
                }}
              />
            </Field>

            <div className="grid grid-cols-2 gap-4">
              <Field
                label={t('floor.table.from')}
                required
                {...fieldErrorProps(fields['from'], common)}
              >
                <Input
                  name="from"
                  inputMode="numeric"
                  autoComplete="off"
                  value={form.from}
                  onChange={(event) => {
                    change('from')(event.target.value)
                  }}
                />
              </Field>
              <Field
                label={t('floor.table.to')}
                required
                {...fieldErrorProps(fields['to'], common)}
              >
                <Input
                  name="to"
                  inputMode="numeric"
                  autoComplete="off"
                  value={form.to}
                  onChange={(event) => {
                    change('to')(event.target.value)
                  }}
                />
              </Field>
            </div>

            <p className="text-sm text-muted-foreground" aria-live="polite">
              {range
                ? t('floor.table.preview', {
                    count: range.count,
                    first: range.first,
                    last: range.last,
                  })
                : t('floor.table.rangeHint')}
            </p>
          </>
        ) : (
          <Field
            label={t('floor.table.label')}
            hint={t('floor.table.labelHint')}
            required
            {...fieldErrorProps(fields['label'], common)}
          >
            <Input
              name="label"
              autoComplete="off"
              value={form.label}
              onChange={(event) => {
                change('label')(event.target.value)
              }}
            />
          </Field>
        )}

        {seatsField}
        {sectionField}
      </form>
    </Dialog>
  )
}
