import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ReceiptText, UtensilsCrossed } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { menuQuery } from '@/shared/api/menu'
import { floorKey, visitKey } from '@/shared/events/query-keys'
import { clockOffset, onDeviceClock } from '@/shared/events/server-clock'
import { formatMoney } from '@/shared/format'
import { Alert } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ElapsedTime } from '@/shared/ui/elapsed-time'
import { EmptyState } from '@/shared/ui/empty-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { StatusPill } from '@/shared/ui/status-pill'
import { showToast } from '@/shared/ui/toast-store'
import { closeVisit, markRoundServed, sendRound, visitQuery, type Round } from '@/waiter/api/orders'

/**
 * One table, for the whole meal: what has been ordered, what is ready, and what
 * it comes to.
 *
 * Three things are happening on this screen at once and it is worth naming
 * them, because feature 12 will pull them apart into a real working surface.
 *
 * **A basket, which lives only in this component.** Nothing about an unsent
 * basket reaches the database. It is state in the browser until the waiter
 * sends it, which is what makes changing your mind free and what makes a
 * refresh mid order lose exactly nothing that mattered.
 *
 * **A record of the meal**, refetched whenever the live stream says something
 * on this visit moved. Every round, its dishes, and the bill's running
 * subtotal, which is true throughout because lines are put on the bill at send.
 *
 * **An alert when food is ready**, once per round. The set of rounds already
 * announced lives in a ref rather than in state, because it must not cause a
 * render and must survive a refetch: without it, every refetch that still shows
 * a ready round would announce it again and a waiter would be shouted at by
 * their own phone.
 */
export function WaiterTable() {
  const { visitId = '' } = useParams()
  const { t } = useTranslation(['waiter', 'common'])
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()

  const visit = useQuery(visitQuery(visitId))
  const menu = useQuery(menuQuery)

  const [basket, setBasket] = useState<Record<string, number>>({})
  const [pending, setPending] = useState<string | null>(null)

  // Rounds this screen has already shouted about. A ref, not state: announcing
  // is a side effect, not something to draw, and a refetch must not re announce.
  const announced = useRef<Set<string>>(new Set())
  const [alerting, setAlerting] = useState<Round | null>(null)

  // The device's clock error, measured against the answer that just arrived.
  // Every age drawn below is corrected by it, so a phone with a wrong clock
  // still shows a true one.
  const offset = useMemo(() => (visit.data ? clockOffset(visit.data.serverTime) : 0), [visit.data])

  const readyRound = visit.data?.rounds.find((round) => round.status === 'ready') ?? null

  useEffect(() => {
    if (!readyRound) return
    if (announced.current.has(readyRound.id)) return

    announced.current.add(readyRound.id)
    setAlerting(readyRound)
  }, [readyRound])

  const send = useMutation({
    mutationFn: () =>
      sendRound(
        visitId,
        Object.entries(basket).map(([dishId, quantity]) => ({ dishId, quantity })),
      ),
    onSuccess: async () => {
      // Cleared only on success. A send that failed leaves the basket exactly
      // as the waiter built it, so retrying is one tap rather than re entering
      // an order they already typed once.
      setBasket({})
      await queryClient.invalidateQueries({ queryKey: visitKey(visitId) })
    },
    onError: (error: unknown) => {
      refuse(error)
    },
  })

  const serve = useMutation({
    mutationFn: (roundId: string) => markRoundServed(roundId),
    onSuccess: async () => {
      setAlerting(null)
      await queryClient.invalidateQueries({ queryKey: visitKey(visitId) })
    },
    onError: (error: unknown) => {
      void queryClient.invalidateQueries({ queryKey: visitKey(visitId) })
      refuse(error)
    },
  })

  const close = useMutation({
    mutationFn: () => closeVisit(visitId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: visitKey(visitId) })
      await queryClient.invalidateQueries({ queryKey: floorKey })
    },
    onError: (error: unknown) => {
      // The common refusal here is `bill_has_unserved_lines`, and the sentence
      // the waiter reads is translated from that code rather than rendered from
      // the API's English.
      void queryClient.invalidateQueries({ queryKey: visitKey(visitId) })
      refuse(error)
    },
  })

  function refuse(error: unknown): void {
    showToast({
      title: apiErrorMessage(failureBody(error), common),
      tone: 'late',
    })
  }

  function add(dishId: string): void {
    setBasket((current) => ({ ...current, [dishId]: (current[dishId] ?? 0) + 1 }))
  }

  function remove(dishId: string): void {
    setBasket((current) => {
      const next = { ...current }
      const left = (next[dishId] ?? 0) - 1
      if (left > 0) next[dishId] = left
      else delete next[dishId]
      return next
    })
  }

  if (visit.isPending) {
    return <Skeleton className="h-96 w-full" label={common('loading.label')} />
  }

  if (visit.isError) {
    return (
      <EmptyState
        icon={UtensilsCrossed}
        title={common('error.title')}
        description={apiErrorMessage(failureBody(visit.error), common)}
        action={
          <Button
            onClick={() => {
              void visit.refetch()
            }}
          >
            {common('error.retry')}
          </Button>
        }
      />
    )
  }

  const { bill } = visit.data
  const closed = visit.data.status === 'closed'
  const basketSize = Object.values(basket).reduce((total, quantity) => total + quantity, 0)

  /** Money on this bill, in the bill's own copied currency, never today's. */
  const billMoney = (amount: string): string =>
    bill ? formatMoney(amount, bill.currencyCode, bill.currencyDecimals) : amount

  return (
    <div className="space-y-6">
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h1 className="text-2xl font-semibold text-foreground">
          {t('table.title', { label: visit.data.tableLabel })}
        </h1>
        <p className="text-sm text-muted-foreground">
          {t('table.openedBy', { name: visit.data.openedBy })}
        </p>
      </header>

      {alerting && (
        <Alert
          open
          tone="ready"
          title={t('alert.title', {
            label: visit.data.tableLabel,
            round: alerting.sequenceNo,
          })}
          description={t('alert.body')}
          onDismiss={() => {
            setAlerting(null)
          }}
        />
      )}

      {!closed && (
        <section aria-labelledby="order-heading" className="space-y-3">
          <h2 id="order-heading" className="text-lg font-medium text-foreground">
            {t('table.orderHeading')}
          </h2>

          {menu.isPending ? (
            <Skeleton className="h-48 w-full" label={common('loading.label')} />
          ) : menu.isError ? (
            <EmptyState icon={UtensilsCrossed} title={common('error.title')} />
          ) : (
            menu.data.categories.map((category) => (
              <Card key={category.id}>
                <h3 className="mb-2 text-sm font-medium text-muted-foreground">{category.name}</h3>
                <ul className="divide-y divide-border">
                  {category.dishes.map((dish) => (
                    <li key={dish.id} className="flex items-center justify-between gap-3 py-2">
                      <div className="min-w-0">
                        <p
                          className={
                            dish.available
                              ? 'truncate text-card-foreground'
                              : 'truncate text-muted-foreground line-through'
                          }
                        >
                          {dish.name}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {formatMoney(
                            dish.price,
                            menu.data.currencyCode,
                            menu.data.currencyDecimals,
                          )}
                          {/* Shown rather than hidden, so a waiter can tell the
                              customer the kitchen has run out. Sending one is
                              refused by the server as well. */}
                          {!dish.available && ` · ${t('table.unavailable')}`}
                        </p>
                      </div>

                      <div className="flex items-center gap-2">
                        {(basket[dish.id] ?? 0) > 0 && (
                          <>
                            <Button
                              variant="secondary"
                              size="sm"
                              aria-label={t('table.removeOne', { dish: dish.name })}
                              onClick={() => {
                                remove(dish.id)
                              }}
                            >
                              −
                            </Button>
                            <span className="tabular w-6 text-center text-sm">
                              {basket[dish.id] ?? 0}
                            </span>
                          </>
                        )}
                        <Button
                          size="sm"
                          disabled={!dish.available}
                          aria-label={t('table.addOne', { dish: dish.name })}
                          onClick={() => {
                            add(dish.id)
                          }}
                        >
                          +
                        </Button>
                      </div>
                    </li>
                  ))}
                </ul>
              </Card>
            ))
          )}

          <Button
            disabled={basketSize === 0 || send.isPending}
            onClick={() => {
              send.mutate()
            }}
          >
            {send.isPending ? t('table.sending') : t('table.send', { count: basketSize })}
          </Button>
        </section>
      )}

      <section aria-labelledby="rounds-heading" className="space-y-3">
        <h2 id="rounds-heading" className="text-lg font-medium text-foreground">
          {t('table.roundsHeading')}
        </h2>

        {visit.data.rounds.length === 0 ? (
          <EmptyState
            icon={UtensilsCrossed}
            title={t('table.nothingSentTitle')}
            description={t('table.nothingSentBody')}
          />
        ) : (
          <ul className="space-y-3">
            {visit.data.rounds.map((round) => (
              <Card as="li" key={round.id} className="space-y-2">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="font-medium text-card-foreground">
                    {t('table.round', { number: round.sequenceNo })}
                  </span>
                  <div className="flex items-center gap-3">
                    <ElapsedTime since={onDeviceClock(round.sentAt, offset)} />
                    <StatusPill status={round.status} />
                  </div>
                </div>

                <ul className="divide-y divide-border text-sm">
                  {round.lines.map((line) => (
                    <li key={line.id} className="flex items-center justify-between gap-3 py-1">
                      <span className="truncate">
                        {line.quantity} × {line.dishName}
                      </span>
                      <span className="flex items-center gap-3">
                        <span className="tabular">{billMoney(line.lineTotal)}</span>
                        <StatusPill status={line.status} compact />
                      </span>
                    </li>
                  ))}
                </ul>

                {round.status === 'ready' && (
                  <Button
                    disabled={serve.isPending}
                    onClick={() => {
                      setPending(round.id)
                      serve.mutate(round.id, { onSettled: () => setPending(null) })
                    }}
                  >
                    {pending === round.id ? t('table.serving') : t('table.serve')}
                  </Button>
                )}
              </Card>
            ))}
          </ul>
        )}
      </section>

      {bill && (
        <Card>
          <h2 className="mb-2 flex items-center gap-2 text-lg font-medium text-card-foreground">
            <ReceiptText aria-hidden="true" className="size-5" />
            {closed ? t('bill.closedTitle', { number: bill.number ?? 0 }) : t('bill.runningTitle')}
          </h2>

          <dl className="space-y-1 text-sm">
            <Figure label={t('bill.subtotal')} value={billMoney(bill.subtotal)} />

            {/* Shown only when there is one. A restaurant that charges no
                service charge should not read a line saying it charged zero. */}
            {bill.serviceChargePercent !== null && (
              <Figure
                label={t('bill.serviceCharge', { percent: bill.serviceChargePercent })}
                value={billMoney(bill.serviceChargeAmount)}
              />
            )}

            {bill.taxes.map((tax) => (
              <Figure
                key={tax.name}
                label={t('bill.tax', { name: tax.name, rate: tax.ratePercent })}
                value={billMoney(tax.amount)}
              />
            ))}

            {closed && <Figure label={t('bill.total')} value={billMoney(bill.total)} emphasis />}
          </dl>

          {!closed && (
            <Button
              className="mt-4"
              disabled={close.isPending}
              onClick={() => {
                close.mutate()
              }}
            >
              {close.isPending ? t('bill.closing') : t('bill.close')}
            </Button>
          )}
        </Card>
      )}
    </div>
  )
}

/** One line of the bill: what it is on the left, what it comes to on the right. */
function Figure({
  label,
  value,
  emphasis = false,
}: {
  label: string
  value: string
  emphasis?: boolean
}) {
  return (
    <div className={emphasis ? 'flex justify-between font-semibold' : 'flex justify-between'}>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="tabular">{value}</dd>
    </div>
  )
}
