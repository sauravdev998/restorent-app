import { createContext, use } from 'react'

/** What a control needs to know to bind itself to the `Field` that wraps it. */
export interface FieldControl {
  /** The id the label points at. */
  id: string
  /** The ids of the hint and the error message, joined, or undefined if neither exists. */
  describedBy: string | undefined
  invalid: boolean
  required: boolean
}

export const FieldContext = createContext<FieldControl | null>(null)

/**
 * Reads the wiring a `Field` set up.
 *
 * Throwing when there is no `Field` is deliberate. A control used bare has no
 * label bound to it, and an unlabelled control is the single most common way a
 * form becomes unusable with a screen reader. Failing loudly during development
 * is much cheaper than shipping it.
 */
export function useFieldControl(): FieldControl {
  const control = use(FieldContext)
  if (!control) {
    throw new Error(
      'This control must be used inside a <Field>, which owns its label and messages.',
    )
  }
  return control
}
