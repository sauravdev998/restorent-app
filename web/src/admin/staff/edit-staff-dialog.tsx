import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'

import {
  changeStaffRole,
  renameStaff,
  staffQuery,
  STAFF_ROLES,
  type StaffMember,
  type StaffRole,
} from '@/admin/api/staff'
import { failureBody, failureCode } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { fieldErrorProps, fieldErrorsFrom, type FieldErrorCode } from '@/shared/api/field-errors'
import { staffKey } from '@/shared/events/query-keys'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'

export interface EditStaffDialogProps {
  onOpenChange: (open: boolean) => void
  /** Who is being changed, as the list last read them. */
  member: StaffMember
  /** Which of the two edits this is. */
  what: 'name' | 'role'
}

/**
 * Changes somebody's name, or their role, in a dialog over the list.
 *
 * Both carry the version the dialog opened with. If somebody changed that
 * person since, the save is refused with `staff_changed`; the dialog then says
 * so, reads the list again, and puts the current value in the control with the
 * current version behind it, so the admin decides on the real value rather than
 * writing over it blind.
 *
 * The two are one component because they are one form with one control swapped.
 * What differs is what they cost: a rename touches no session, and a role
 * change signs that person out of every device they hold, which the dialog says
 * before it is done rather than after.
 */
export function EditStaffDialog({ onOpenChange, member, what }: EditStaffDialogProps) {
  const { t } = useTranslation('admin')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const formId = useId()

  const [displayName, setDisplayName] = useState(member.displayName)
  const [role, setRole] = useState<StaffRole>(member.role)
  const [version, setVersion] = useState(member.version)
  // The role this person held at `version`, so a save to that same role can
  // say it changed nothing: the API accepts it and writes nothing, revoking no
  // session, and "signed out everywhere" would be untrue.
  const [heldRole, setHeldRole] = useState<StaffRole>(member.role)
  const [fields, setFields] = useState<Record<string, FieldErrorCode>>({})
  const [problem, setProblem] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: (submitted: { role: StaffRole; heldRole: StaffRole }) =>
      what === 'name'
        ? renameStaff(member.id, displayName, version)
        : changeStaffRole(member.id, submitted.role, version),
    onSuccess: async (saved, submitted) => {
      await queryClient.invalidateQueries({ queryKey: staffKey })
      const roleArgs = { name: saved.displayName, role: t(`staff.roles.${saved.role}`) }
      showToast({
        title:
          what === 'name'
            ? t('staff.edit.renamed', { name: saved.displayName })
            : submitted.role === submitted.heldRole
              ? t('staff.edit.roleUnchanged', roleArgs)
              : t('staff.edit.roleChanged', roleArgs),
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

      // Stale: show what this person is now, and save against that.
      if (failureCode(error) === 'staff_changed') {
        const people = await queryClient.fetchQuery({ ...staffQuery, staleTime: 0 })
        const current = people.find((each) => each.id === member.id)
        if (current) {
          setDisplayName(current.displayName)
          setRole(current.role)
          setHeldRole(current.role)
          setVersion(current.version)
        }
        return
      }

      // Every other refusal is about a state the list does not show correctly
      // any more, such as an account somebody switched off meanwhile.
      void queryClient.invalidateQueries({ queryKey: staffKey })
    },
  })

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    setFields({})
    setProblem(null)
    save.mutate({ role, heldRole })
  }

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      title={
        what === 'name'
          ? t('staff.edit.nameTitle', { name: member.displayName })
          : t('staff.edit.roleTitle', { name: member.displayName })
      }
      description={what === 'name' ? t('staff.edit.nameBody') : t('staff.edit.roleBody')}
      footer={
        <>
          <Button type="submit" form={formId} disabled={save.isPending}>
            {save.isPending ? t('staff.saving') : t('staff.edit.submit')}
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            {t('staff.cancel')}
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

        {what === 'name' ? (
          <Field
            label={t('staff.name')}
            hint={t('staff.nameHint')}
            required
            {...fieldErrorProps(fields['displayName'], common)}
          >
            <Input
              name="displayName"
              autoComplete="off"
              value={displayName}
              onChange={(event) => {
                setDisplayName(event.target.value)
              }}
            />
          </Field>
        ) : (
          <Field label={t('staff.role')} hint={t('staff.edit.roleHint')} required>
            <Select
              name="role"
              value={role}
              onChange={(event) => {
                setRole(event.target.value as StaffRole)
              }}
            >
              {STAFF_ROLES.map((each) => (
                <option key={each} value={each}>
                  {t(`staff.roles.${each}`)}
                </option>
              ))}
            </Select>
          </Field>
        )}
      </form>
    </Dialog>
  )
}
