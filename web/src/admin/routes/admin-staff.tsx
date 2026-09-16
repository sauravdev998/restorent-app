import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { UserPlus, Users } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { deactivateStaff, reactivateStaff, staffQuery, type StaffMember } from '@/admin/api/staff'
import { ConfirmDialog } from '@/admin/confirm-dialog'
import { CreateStaffDialog } from '@/admin/staff/create-staff-dialog'
import { EditStaffDialog } from '@/admin/staff/edit-staff-dialog'
import { ResetPasswordDialog } from '@/admin/staff/reset-password-dialog'
import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { staffKey } from '@/shared/events/query-keys'
import { formatTimestamp } from '@/shared/format'
import { useIdentity } from '@/shared/session/use-identity'
import { Button } from '@/shared/ui/button'
import { DataTable, type DataTableColumn } from '@/shared/ui/data-table'
import { EmptyState } from '@/shared/ui/empty-state'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'
import { showToast } from '@/shared/ui/toast-store'

/** Which dialog is open, and who it was opened from. */
type Opened =
  | { kind: 'create' }
  | { kind: 'name'; member: StaffMember }
  | { kind: 'role'; member: StaffMember }
  | { kind: 'reset'; member: StaffMember }
  | { kind: 'deactivate'; member: StaffMember }
  | null

/**
 * The admin's staff: everybody who works here, and the six things an admin does
 * to an account that is not their own.
 *
 * One page, two lists. Active people are the working list, and switched off
 * people sit below in a section of their own where the only thing on offer is
 * bringing them back, because that is genuinely all there is to do with a row
 * that nobody may sign in as.
 *
 * **The admin's own row carries no actions.** Their name, their language, and
 * their password live on the account screen, behind their current password, and
 * the API refuses all three of them here whatever this screen draws. Hiding the
 * buttons is so that nothing on the page invites a refusal.
 *
 * **Nothing is written into the cache ahead of the server.** Every write
 * invalidates the list on success and reads it again. Two of them, the role
 * change and the reset, also end every session that person holds, which is
 * something no response body can show, and one, a role change to the role
 * somebody already has, deliberately writes nothing at all.
 *
 * **Nobody else's screen learns of a change from an event**, and that is a
 * decision rather than a gap. A staff change reaches the person it is about the
 * only way that matters: their session stops working, on their very next
 * request. A second admin with this page open sees a stale list until they act,
 * and then reads a refusal that says what happened.
 */
export function AdminStaffScreen() {
  const { t } = useTranslation(['admin', 'common'])
  const { t: common } = useTranslation()

  const identity = useIdentity()
  const queryClient = useQueryClient()
  const people = useQuery(staffQuery)
  const [opened, setOpened] = useState<Opened>(null)

  // Bringing somebody back is the one action with no dialog: it asks nothing
  // and changes nothing but the one column, so a confirmation would be a step
  // for its own sake. It still has to say when it is refused, which is what a
  // bare promise in an `onClick` would swallow.
  const bringBack = useMutation({
    mutationFn: (member: StaffMember) => reactivateStaff(member.id),
    onSuccess: async (member) => {
      await queryClient.invalidateQueries({ queryKey: staffKey })
      showToast({ title: t('staff.reactivate.done', { name: member.displayName }) })
    },
    onError: async (error: unknown) => {
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
      await queryClient.invalidateQueries({ queryKey: staffKey })
    },
  })

  const heading = (
    <div className="max-w-prose">
      <h1 className="text-2xl font-semibold text-foreground">{t('staff.title')}</h1>
      <p className="mt-1 text-sm text-muted-foreground">{t('staff.intro')}</p>
    </div>
  )

  if (people.isPending) {
    return (
      <div className="space-y-6">
        {heading}
        <Skeleton className="h-64 w-full" label={common('loading.label')} />
      </div>
    )
  }

  if (people.isError) {
    return (
      <div className="space-y-6">
        {heading}
        <EmptyState
          icon={Users}
          title={common('error.title')}
          description={apiErrorMessage(failureBody(people.error), common)}
          action={
            <Button
              onClick={() => {
                void people.refetch()
              }}
            >
              {common('error.retry')}
            </Button>
          }
        />
      </div>
    )
  }

  const active = people.data.filter((member) => member.active)
  const inactive = people.data.filter((member) => !member.active)
  const owing = active.filter((member) => member.mustChangePassword).length

  /**
   * When somebody last signed in, or the word for never.
   *
   * `?? null` rather than a check against one of the two: the API says "never"
   * by leaving the member out, and the generated type is therefore optional as
   * well as nullable, so a check against `null` alone would leave `undefined`
   * to be formatted as a date.
   */
  function signedIn(member: StaffMember): string {
    const at = member.lastSignInAt ?? null
    return at === null ? t('staff.never') : formatTimestamp(at, 'dateTime')
  }

  const columns: readonly DataTableColumn<StaffMember>[] = [
    {
      key: 'name',
      header: t('staff.name'),
      rowHeader: true,
      cell: (member) => (
        <div className="flex flex-col gap-1">
          {/* Somebody's name is data, not copy: it is written in whatever
              language they wrote it in, and a screen reader has to be told
              which so it does not read it with the wrong phonetics. */}
          <RestaurantText className="font-medium text-foreground">
            {member.displayName}
          </RestaurantText>
          <span className="text-xs break-all text-muted-foreground">{member.email}</span>
        </div>
      ),
    },
    {
      key: 'role',
      header: t('staff.role'),
      cell: (member) => t(`staff.roles.${member.role}`),
    },
    {
      key: 'lastSignIn',
      header: t('staff.lastSignIn'),
      cell: (member) => (
        <div className="flex flex-col gap-1">
          <span
            className={(member.lastSignInAt ?? null) === null ? 'text-muted-foreground' : undefined}
          >
            {signedIn(member)}
          </span>
          {/* Not a status colour. `design.md` spends colour on order status and
              on the three diet marks and nowhere else, and this is neither: it
              is a note an admin reads once. Weight and the words carry it. */}
          {member.mustChangePassword && (
            <span className="text-xs font-medium text-foreground">{t('staff.owesPassword')}</span>
          )}
        </div>
      ),
    },
    {
      key: 'actions',
      header: t('staff.actions'),
      cell: (member) =>
        member.id === identity.staff.id ? (
          <span className="text-xs text-muted-foreground">{t('staff.you')}</span>
        ) : (
          <div className="flex flex-wrap gap-2">
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('staff.edit.nameFor', { name: member.displayName })}
              onClick={() => {
                setOpened({ kind: 'name', member })
              }}
            >
              {t('staff.edit.name')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('staff.edit.roleFor', { name: member.displayName })}
              onClick={() => {
                setOpened({ kind: 'role', member })
              }}
            >
              {t('staff.edit.role')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('staff.reset.for', { name: member.displayName })}
              onClick={() => {
                setOpened({ kind: 'reset', member })
              }}
            >
              {t('staff.reset.action')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('staff.deactivate.for', { name: member.displayName })}
              onClick={() => {
                setOpened({ kind: 'deactivate', member })
              }}
            >
              {t('staff.deactivate.action')}
            </Button>
          </div>
        ),
    },
  ]

  const inactiveColumns: readonly DataTableColumn<StaffMember>[] = [
    {
      key: 'name',
      header: t('staff.name'),
      rowHeader: true,
      cell: (member) => (
        <div className="flex flex-col gap-1">
          <RestaurantText className="font-medium text-foreground">
            {member.displayName}
          </RestaurantText>
          <span className="text-xs break-all text-muted-foreground">{member.email}</span>
        </div>
      ),
    },
    {
      key: 'role',
      header: t('staff.role'),
      cell: (member) => t(`staff.roles.${member.role}`),
    },
    {
      key: 'actions',
      header: t('staff.actions'),
      cell: (member) => (
        <Button
          variant="secondary"
          size="sm"
          disabled={bringBack.isPending}
          aria-label={t('staff.reactivate.for', { name: member.displayName })}
          onClick={() => {
            bringBack.mutate(member)
          }}
        >
          {t('staff.reactivate.action')}
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="space-y-2">
          {heading}
          <p className="text-sm text-muted-foreground">
            {[
              t('staff.activeCount', { count: active.length }),
              ...(owing > 0 ? [t('staff.owingCount', { count: owing })] : []),
              ...(inactive.length > 0
                ? [t('staff.inactiveCount', { count: inactive.length })]
                : []),
            ].join(' · ')}
          </p>
        </div>

        <Button
          onClick={() => {
            setOpened({ kind: 'create' })
          }}
        >
          {t('staff.create.action')}
        </Button>
      </header>

      <section aria-labelledby="staff-active-heading" className="space-y-3">
        <h2 id="staff-active-heading" className="text-xl font-semibold text-foreground">
          {t('staff.activeTitle')}
        </h2>

        {active.length === 0 ? (
          <EmptyState
            icon={UserPlus}
            title={t('staff.emptyTitle')}
            description={t('staff.emptyBody')}
            action={
              <Button
                onClick={() => {
                  setOpened({ kind: 'create' })
                }}
              >
                {t('staff.create.action')}
              </Button>
            }
          />
        ) : (
          <DataTable
            caption={t('staff.activeCaption')}
            captionHidden
            columns={columns}
            rows={active}
            rowKey={(member) => member.id}
          />
        )}
      </section>

      {inactive.length > 0 && (
        <section aria-labelledby="staff-inactive-heading" className="space-y-3">
          <h2 id="staff-inactive-heading" className="text-xl font-semibold text-foreground">
            {t('staff.inactiveTitle')}
          </h2>
          <p className="max-w-prose text-sm text-muted-foreground">{t('staff.inactiveBody')}</p>

          <DataTable
            caption={t('staff.inactiveCaption')}
            captionHidden
            columns={inactiveColumns}
            rows={inactive}
            rowKey={(member) => member.id}
          />
        </section>
      )}

      {opened?.kind === 'create' && (
        <CreateStaffDialog
          onOpenChange={(open) => {
            if (!open) setOpened(null)
          }}
        />
      )}

      {(opened?.kind === 'name' || opened?.kind === 'role') && (
        <EditStaffDialog
          key={`${opened.kind}-${opened.member.id}-${String(opened.member.version)}`}
          onOpenChange={(open) => {
            if (!open) setOpened(null)
          }}
          member={opened.member}
          what={opened.kind}
        />
      )}

      {opened?.kind === 'reset' && (
        <ResetPasswordDialog
          onOpenChange={(open) => {
            if (!open) setOpened(null)
          }}
          member={opened.member}
        />
      )}

      {opened?.kind === 'deactivate' && (
        <ConfirmDialog
          onOpenChange={(open) => {
            if (!open) setOpened(null)
          }}
          title={t('staff.deactivate.title', { name: opened.member.displayName })}
          description={t('staff.deactivate.body')}
          confirmLabel={t('staff.deactivate.confirm')}
          doneMessage={t('staff.deactivate.done', { name: opened.member.displayName })}
          action={() => deactivateStaff(opened.member.id)}
          invalidateKey={staffKey}
        />
      )}
    </div>
  )
}
