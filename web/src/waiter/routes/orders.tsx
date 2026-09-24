import { useQuery } from '@tanstack/react-query'
import { ClipboardList } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { clockOffset, onDeviceClock } from '@/shared/events/server-clock'
import { useIdentity } from '@/shared/session/use-identity'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ElapsedTime } from '@/shared/ui/elapsed-time'
import { EmptyState } from '@/shared/ui/empty-state'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'
import { StatusPill } from '@/shared/ui/status-pill'
import { readyCount, sortOrders } from '@/waiter/alerts/ready'
import { openOrdersQuery, type OpenOrder } from '@/waiter/api/orders'
import { ReadyBadge } from '@/waiter/components/ready-badge'
import { WaiterViews } from '@/waiter/components/waiter-views'
import { useMineOnly } from '@/waiter/mine-only'
import { useServeLine, useTakeOver } from '@/waiter/use-waiter-actions'

/**
 * Every open table in the restaurant and where each of its rounds has got to,
 * in the order a waiter should walk (spec 0011, AC-2).
 *
 * Tables with food on the pass first, the one that has waited longest at the
 * top; then tables with something still cooking; then the rest. A ready dish
 * can be carried out from right here, one at a time, without opening the
 * table (AC-12), which is what a waiter with three plates in hand needs.
 *
 * The list is the same query the ready alert reads, refetched by the live
 * stream whenever any dish anywhere moves, so it updates with no refresh and
 * never disagrees with the alert. Every age is corrected by the server's own
 * clock, so a phone set wrong still shows true minutes.
 */
export function WaiterOrders() {
  const { t } = useTranslation(['waiter', 'common'])
  const { t: common } = useTranslation()
  const me = useIdentity().staff.id
  const mineOnly = useMineOnly()

  const orders = useQuery(openOrdersQuery)
  const { serving, serve } = useServeLine()
  const { taking, takeOver } = useTakeOver()

  const offset = useMemo(
    () => (orders.data ? clockOffset(orders.data.serverTime) : 0),
    [orders.data],
  )

  const shown = useMemo(() => {
    if (!orders.data) return []
    const visits = mineOnly
      ? orders.data.visits.filter((visit) => visit.responsibleStaffId === me)
      : orders.data.visits
    return sortOrders(visits)
  }, [orders.data, mineOnly, me])

  const heading = (
    <>
      <WaiterViews />
      <h1 className="text-2xl font-semibold text-foreground">{t('orders.title')}</h1>
    </>
  )

  if (orders.isPending) {
    return (
      <div className="space-y-4">
        {heading}
        <Skeleton className="h-64 w-full" label={common('loading.label')} />
      </div>
    )
  }

  if (orders.isError) {
    return (
      <div className="space-y-4">
        {heading}
        <EmptyState
          icon={ClipboardList}
          title={common('error.title')}
          description={apiErrorMessage(failureBody(orders.error), common)}
          action={
            <Button
              onClick={() => {
                void orders.refetch()
              }}
            >
              {common('error.retry')}
            </Button>
          }
        />
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {heading}

      {shown.length === 0 ? (
        <EmptyState
          icon={ClipboardList}
          title={mineOnly ? t('orders.emptyMineTitle') : t('orders.emptyTitle')}
          description={mineOnly ? t('orders.emptyMineBody') : t('orders.emptyBody')}
        />
      ) : (
        <ul className="grid items-start gap-3 lg:grid-cols-2">
          {shown.map((order) => (
            <OrderCard
              key={order.id}
              order={order}
              mine={order.responsibleStaffId === me}
              offset={offset}
              serving={serving}
              onServe={serve}
              taking={taking === order.id}
              onTakeOver={() => {
                takeOver(order.id, order.responsibleStaffId)
              }}
            />
          ))}
        </ul>
      )}
    </div>
  )
}

interface OrderCardProps {
  order: OpenOrder
  mine: boolean
  offset: number
  serving: string | null
  onServe: (lineId: string) => void
  taking: boolean
  onTakeOver: () => void
}

/** One open table: who has it, and every round on it with what is ready. */
function OrderCard({ order, mine, offset, serving, onServe, taking, onTakeOver }: OrderCardProps) {
  const { t } = useTranslation('waiter')
  const ready = readyCount(order)

  return (
    <Card as="li" className="space-y-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <h2 className="text-lg font-semibold text-card-foreground">
            <Link to={`/waiter/tables/${order.id}`} className="underline-offset-4 hover:underline">
              {t('orders.table', { label: order.tableLabel })}
            </Link>
          </h2>
          <p className="text-xs text-muted-foreground">
            {mine ? t('responsible.you') : t('responsible.named', { name: order.responsibleName })}
          </p>
        </div>
        <ReadyBadge count={ready} />
      </div>

      {order.rounds.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('orders.nothingSent')}</p>
      ) : (
        <ol className="space-y-3">
          {order.rounds.map((round) => (
            <li key={round.id} className="border-line space-y-2 rounded-md border-border p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-sm font-medium text-card-foreground">
                  {t('table.round', { number: round.sequenceNo })}
                </span>
                <span className="flex items-center gap-3">
                  <ElapsedTime since={onDeviceClock(round.sentAt, offset)} />
                  <StatusPill status={round.status} />
                </span>
              </div>

              <ul className="divide-y divide-border text-sm">
                {round.lines.map((line) => (
                  <li key={line.id} className="flex items-center justify-between gap-3 py-2">
                    <div className="min-w-0">
                      <p
                        className={
                          line.status === 'voided' ? 'text-muted-foreground line-through' : ''
                        }
                      >
                        {line.quantity} × <RestaurantText>{line.dishName}</RestaurantText>
                      </p>
                      {line.note !== null && (
                        <RestaurantText
                          as="p"
                          className="text-xs break-words whitespace-pre-wrap text-muted-foreground"
                        >
                          {line.note}
                        </RestaurantText>
                      )}
                    </div>
                    {line.status === 'ready' ? (
                      <Button
                        size="sm"
                        disabled={serving === line.id}
                        aria-label={t('serve.oneNamed', { dish: line.dishName })}
                        onClick={() => {
                          onServe(line.id)
                        }}
                      >
                        {serving === line.id ? t('table.serving') : t('serve.one')}
                      </Button>
                    ) : (
                      <StatusPill status={line.status} compact />
                    )}
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ol>
      )}

      {!mine && (
        <Button variant="secondary" size="sm" disabled={taking} onClick={onTakeOver}>
          {taking ? t('takeOver.taking') : t('takeOver.action')}
        </Button>
      )}
    </Card>
  )
}
