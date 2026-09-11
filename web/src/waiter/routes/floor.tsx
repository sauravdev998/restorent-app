import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { LayoutGrid } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { floorKey } from '@/shared/events/query-keys'
import { formatTimestamp } from '@/shared/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { StatusPill } from '@/shared/ui/status-pill'
import { showToast } from '@/shared/ui/toast-store'
import { floorQuery, openVisit, type FloorTable } from '@/waiter/api/orders'

/**
 * The waiter's landing screen: the whole floor, free tables and taken ones.
 *
 * Deliberately plain. Feature 12 builds the real working screen a waiter uses
 * all evening; this one exists to prove the thread and to be usable while it
 * does, so it is a list of tables with one action on each.
 *
 * Tapping a free table opens it and walks straight into the ordering screen,
 * because that is what a waiter is doing: they are not opening a table for its
 * own sake, they are taking an order. Tapping a taken one goes to the same
 * screen without writing anything.
 *
 * Any waiter may act on any table. The screen names whoever opened it, because
 * that is worth knowing at a shift change, but it is a fact rather than a rule
 * and the server does not restrict on it either.
 */
export function WaiterFloor() {
  const { t } = useTranslation(['waiter', 'common'])
  const { t: common } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const floor = useQuery(floorQuery)

  // Which table is mid open, so its own button shows the wait rather than the
  // whole screen going blank. There is no optimistic write anywhere near this:
  // the table is occupied when the server says it is, and pretending otherwise
  // is exactly how two waiters both believe they have table four.
  const [opening, setOpening] = useState<string | null>(null)

  const open = useMutation({
    mutationFn: (tableId: string) => openVisit(tableId),
    onSuccess: async (visitId) => {
      // Walk to the table first, then refresh the floor behind us. Awaiting the
      // refetch before navigating would hold a waiter standing at a table
      // watching a spinner while the screen they are leaving reloads.
      void queryClient.invalidateQueries({ queryKey: floorKey })
      await navigate(`/waiter/tables/${visitId}`)
    },
    onError: (error: unknown) => {
      // A refusal is almost always `table_occupied`: somebody opened it first.
      // The floor is refetched so the screen shows where things really stand
      // rather than the state the tap assumed.
      void queryClient.invalidateQueries({ queryKey: floorKey })

      showToast({
        title: apiErrorMessage(failureBody(error), common),
        tone: 'late',
      })
    },
    onSettled: () => {
      setOpening(null)
    },
  })

  function handleOpen(table: FloorTable): void {
    if (table.occupancy) {
      void navigate(`/waiter/tables/${table.occupancy.visitId}`)
      return
    }

    setOpening(table.id)
    open.mutate(table.id)
  }

  if (floor.isPending) {
    return (
      <div className="space-y-4">
        <h1 className="text-2xl font-semibold text-foreground">{t('floor.title')}</h1>
        <Skeleton className="h-64 w-full" label={common('loading.label')} />
      </div>
    )
  }

  if (floor.isError) {
    return (
      <div className="space-y-4">
        <h1 className="text-2xl font-semibold text-foreground">{t('floor.title')}</h1>
        <EmptyState
          icon={LayoutGrid}
          title={common('error.title')}
          description={apiErrorMessage(failureBody(floor.error), common)}
          action={
            <Button
              onClick={() => {
                void floor.refetch()
              }}
            >
              {common('error.retry')}
            </Button>
          }
        />
      </div>
    )
  }

  const empty = floor.data.sections.every((section) => section.tables.length === 0)

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold text-foreground">{t('floor.title')}</h1>

      {empty ? (
        <EmptyState
          icon={LayoutGrid}
          title={t('floor.emptyTitle')}
          description={t('floor.emptyBody')}
        />
      ) : (
        floor.data.sections.map((section) => (
          <section key={section.id ?? 'unsectioned'} aria-labelledby={`s-${section.id ?? 'none'}`}>
            <h2
              id={`s-${section.id ?? 'none'}`}
              className="mb-2 text-sm font-medium text-muted-foreground"
            >
              {/* A table can belong to no section, or to one that has since been
                  archived. Either way it is still a table, and the heading it
                  goes under is this screen's word rather than a database row. */}
              {section.name ?? t('floor.unsectioned')}
            </h2>

            <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {section.tables.map((table) => (
                <Card as="li" key={table.id} className="flex flex-col gap-3">
                  <div className="flex items-start justify-between gap-2">
                    <span className="text-lg font-semibold text-card-foreground">
                      {table.label}
                    </span>
                    {table.occupancy?.foodReady === true && <StatusPill status="ready" compact />}
                  </div>

                  {table.occupancy ? (
                    <p className="text-xs text-muted-foreground">
                      {t('floor.openedBy', {
                        name: table.occupancy.openedBy,
                        at: formatTimestamp(table.occupancy.openedAt, 'time'),
                      })}
                    </p>
                  ) : (
                    <p className="text-xs text-muted-foreground">{t('floor.free')}</p>
                  )}

                  <Button
                    variant={table.occupancy ? 'secondary' : 'primary'}
                    disabled={opening === table.id}
                    onClick={() => {
                      handleOpen(table)
                    }}
                  >
                    {opening === table.id
                      ? t('floor.opening')
                      : table.occupancy
                        ? t('floor.view')
                        : t('floor.open')}
                  </Button>
                </Card>
              ))}
            </ul>
          </section>
        ))
      )}
    </div>
  )
}
