import { useId, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from './cn'
import { FieldContext, type FieldControl } from './field-context'

export interface FieldProps {
  /** The visible label. Always visible: a placeholder is not a label. */
  label: string
  /** Exactly one control, an `Input` or a `Select`. */
  children: ReactNode
  /** Help text shown under the control and read out with it. */
  hint?: string
  /** The validation message. Its presence is what marks the control invalid. */
  error?: string
  required?: boolean
  className?: string
}

/**
 * A labelled form control, with its hint and its error wired to it.
 *
 * The wiring is the whole reason this exists. The label points at the control
 * by id, the hint and the error are joined into `aria-describedby` so a screen
 * reader reads them as part of the control rather than as stray text somewhere
 * on the page, and the error also announces itself the moment it appears
 * instead of only becoming visible to people who happen to be looking at it.
 */
export function Field({ label, children, hint, error, required = false, className }: FieldProps) {
  const { t } = useTranslation()
  const id = useId()
  const hintId = `${id}-hint`
  const errorId = `${id}-error`

  const describedBy =
    [hint === undefined ? null : hintId, error === undefined ? null : errorId]
      .filter((value) => value !== null)
      .join(' ') || undefined

  const control: FieldControl = {
    id,
    describedBy,
    invalid: error !== undefined,
    required,
  }

  return (
    <div className={cn('flex flex-col gap-2', className)}>
      <label htmlFor={id} className="flex items-center gap-2 text-sm font-medium text-foreground">
        {label}
        {required && (
          <span className="text-xs font-normal text-muted-foreground">{t('field.required')}</span>
        )}
      </label>

      <FieldContext value={control}>{children}</FieldContext>

      {hint !== undefined && (
        <p id={hintId} className="text-xs text-muted-foreground">
          {hint}
        </p>
      )}

      {error !== undefined && (
        <p id={errorId} role="alert" className="text-xs font-medium text-status-late">
          {error}
        </p>
      )}
    </div>
  )
}
