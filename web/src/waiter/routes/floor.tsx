import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { LayoutGrid } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { floorKey } from '@/shared/events/query-keys'
import { formatTimestamp } from '@/shared/format'
import { useIdentity } from '@/shared/session/use-identity'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'
import { showToast } from '@/shared/ui/toast-store'
import { floorQuery, openVisit, type FloorTable } from '@/waiter/api/orders'
import { ReadyBadge } from '@/waiter/components/ready-badge'
import { WaiterViews } from '@/waiter/components/waiter-views'
import { useMineOnly } from '@/waiter/mine-only'

/**
 * The waiter's landing screen: the whole floor, free tables and taken ones.
 *
 * One of the waiter's two views, beside Orders (spec 0011, AC-1), and the one
 * a waiter lands on. Each occupied table names its responsible waiter, the
 * one who hears its ready chime, and carries a badge counting the dishes
 * waiting on the pass. The Mine filter narrows it to the tables that are
 * mine; free tables stay, because any waiter may seat a party.
 *
 * Tapping a free table opens it and walks straight into the ordering screen,
 * because that is what a waiter is doing: they are not opening a table for its
 * own sake, they are taking an order. Tapping a taken one goes to the same
 * screen without writing anything.
 *
 * Any waiter may act on any table. Responsibility decides who hears the chime,
 * not who may act, and the server does not restrict on it either.
 */
export function WaiterFloor() {
  const { t } = useTranslation(['waiter', 'common'])
  const { t: common } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const me = useIdentity().staff.id
  const mineOnly = useMineOnly()

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
        <WaiterViews />
        <h1 className="text-2xl font-semibold text-foreground">{t('floor.title')}</h1>
        <Skeleton className="h-64 w-full" label={common('loading.label')} />
      </div>
    )
  }

  if (floor.isError) {
    return (
      <div className="space-y-4">
        <WaiterViews />
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

  // Mine hides the occupied tables somebody else is responsible for. Free
  // tables stay: seating a party is anybody's to do.
  const sections = floor.data.sections
    .map((section) => ({
      ...section,
      tables: mineOnly
        ? section.tables.filter(
            (table) => !table.occupancy || table.occupancy.responsibleStaffId === me,
          )
        : section.tables,
    }))
    .filter((section) => section.tables.length > 0)

  return (
    <div className="space-y-6">
      <WaiterViews />
      <h1 className="text-2xl font-semibold text-foreground">{t('floor.title')}</h1>

      {empty ? (
        <EmptyState
          icon={LayoutGrid}
          title={t('floor.emptyTitle')}
          description={t('floor.emptyBody')}
        />
      ) : sections.length === 0 ? (
        <EmptyState
          icon={LayoutGrid}
          title={t('floor.emptyMineTitle')}
          description={t('floor.emptyMineBody')}
        />
      ) : (
        sections.map((section) => (
          <section key={section.id ?? 'unsectioned'} aria-labelledby={`s-${section.id ?? 'none'}`}>
            <h2
              id={`s-${section.id ?? 'none'}`}
              className="mb-2 text-sm font-medium text-muted-foreground"
            >
              {/* A table can belong to no section, or to one that has since been
                  archived. Either way it is still a table, and the heading it
                  goes under is this screen's word rather than a database row. */}
              {section.name === null ? (
                t('floor.unsectioned')
              ) : (
                <RestaurantText>{section.name}</RestaurantText>
              )}
            </h2>

            <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {section.tables.map((table) => (
                <Card as="li" key={table.id} className="flex flex-col gap-3">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <RestaurantText
                        as="p"
                        className="truncate text-lg font-semibold text-card-foreground"
                      >
                        {table.label}
                      </RestaurantText>
                      {table.seats !== null && (
                        <p className="text-xs text-muted-foreground">
                          {t('floor.seats', { count: table.seats })}
                        </p>
                      )}
                    </div>
                    <ReadyBadge count={table.occupancy?.readyDishCount ?? 0} className="shrink-0" />
                  </div>

                  {table.occupancy ? (
                    <div className="space-y-1 text-xs text-muted-foreground">
                      <p className="font-medium text-card-foreground">
                        {table.occupancy.responsibleStaffId === me
                          ? t('responsible.you')
                          : t('responsible.named', { name: table.occupancy.responsibleName })}
                      </p>
                      <p>
                        {t('floor.openedAt', {
                          at: formatTimestamp(table.occupancy.openedAt, 'time'),
                        })}
                      </p>
                    </div>
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
