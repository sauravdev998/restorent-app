import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { restaurantLanguage } from '@/shared/session/identity'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Field } from '@/shared/ui/field'
import { Select } from '@/shared/ui/select'
import { showToast } from '@/shared/ui/toast-store'
import { floorQuery, moveVisit } from '@/waiter/api/orders'

export interface MoveDialogProps {
  /** Whether it is open. */
  open: boolean
  onClose: () => void
  /** The party being moved. */
  visitId: string
  /** Where they sit now, for the dialog's own words. */
  tableLabel: string
}

/**
 * Moving a party to a free table (spec 0011, AC-15).
 *
 * Offers only the free live tables the floor read shows, so the common mistake
 * cannot be made. The party takes everything with it: its rounds, its bill,
 * its responsible waiter, and the basket saved on this phone, which is kept
 * against the visit rather than the table. The kitchen's tickets read the new
 * label as soon as the move lands.
 *
 * A table a colleague sat somebody at a moment ago is refused with
 * `table_occupied`, and the floor is refetched so the list is true again.
 */
export function MoveDialog({ open, onClose, visitId, tableLabel }: MoveDialogProps) {
  const { t } = useTranslation('waiter')
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const floor = useQuery({ ...floorQuery, enabled: open })
  const [picked, setPicked] = useState('')

  const free = (floor.data?.sections ?? []).flatMap((section) =>
    section.tables.filter((table) => table.occupancy === null),
  )

  const move = useMutation({
    mutationFn: (tableId: string) => moveVisit(visitId, tableId),
    onSuccess: async () => {
      showToast({ title: t('move.done'), tone: 'ready' })
      close()
      await queryClient.invalidateQueries({ queryKey: ['visit'] })
    },
    onError: (error: unknown) => {
      void queryClient.invalidateQueries({ queryKey: ['visit'] })
      showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
    },
  })

  function close(): void {
    setPicked('')
    onClose()
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) close()
      }}
      title={t('move.title', { label: tableLabel })}
      description={t('move.description')}
      footer={
        <>
          <Button variant="secondary" onClick={close}>
            {t('move.keep')}
          </Button>
          <Button
            disabled={picked === '' || move.isPending}
            onClick={() => {
              if (picked !== '') move.mutate(picked)
            }}
          >
            {move.isPending ? t('move.moving') : t('move.confirm')}
          </Button>
        </>
      }
    >
      {free.length === 0 && !floor.isPending ? (
        <p className="text-sm text-muted-foreground">{t('move.noneFree')}</p>
      ) : (
        <Field label={t('move.tableLabel')}>
          <Select
            value={picked}
            onChange={(event) => {
              setPicked(event.target.value)
            }}
          >
            <option value="">{t('move.choose')}</option>
            {free.map((table) => (
              <option key={table.id} value={table.id} lang={restaurantLanguage()}>
                {table.label}
              </option>
            ))}
          </Select>
        </Field>
      )}
    </Dialog>
  )
}
