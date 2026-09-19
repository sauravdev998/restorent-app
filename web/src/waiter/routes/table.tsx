import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowRightLeft, ReceiptText, UtensilsCrossed } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router'

import { failureBody, failureCode } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { menuQuery } from '@/shared/api/menu'
import { menuKey } from '@/shared/events/query-keys'
import { clockOffset, onDeviceClock } from '@/shared/events/server-clock'
import { formatMoney } from '@/shared/format'
import { useIdentity } from '@/shared/session/use-identity'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { DietMark } from '@/shared/ui/diet-mark'
import { ElapsedTime } from '@/shared/ui/elapsed-time'
import { EmptyState } from '@/shared/ui/empty-state'
import { Icon } from '@/shared/ui/icon'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { Skeleton } from '@/shared/ui/skeleton'
import { StatusPill } from '@/shared/ui/status-pill'
import { showToast } from '@/shared/ui/toast-store'
import { acknowledge } from '@/waiter/alerts/acknowledged'
import {
  closeVisit,
  markRoundServed,
  sendRound,
  visitQuery,
  type OrderLine,
  type Round,
} from '@/waiter/api/orders'
import {
  addOne,
  basketSize,
  noteTooLong,
  sendLines,
  unorderableDishes,
  type SendLine,
} from '@/waiter/basket'
import { BasketPanel } from '@/waiter/components/basket-panel'
import { MoveDialog } from '@/waiter/components/move-dialog'
import { VoidDialog } from '@/waiter/components/void-dialog'
import { WaiterViews } from '@/waiter/components/waiter-views'
import { useSavedBasket } from '@/waiter/use-saved-basket'
import { useServeLine, useTakeOver } from '@/waiter/use-waiter-actions'

/**
 * One table, for the whole meal: what to send, what is cooking, what is ready,
 * and what it comes to.
 *
 * **The basket survives.** It is saved per visit on this phone as it is built,
 * so leaving for another table or reloading loses nothing, and it carries the
 * key that makes tapping Send again after a timeout safe (spec 0011, AC-7,
 * AC-8). It is cleared once a send lands, whether it made the ticket or found
 * the one an earlier try made, and dropped with a word of explanation if the
 * table has closed meanwhile.
 *
 * **Every round is its own ticket** with its own status (AC-5). A ready dish
 * is carried out on its own, or every ready dish on a round at once, even
 * while the rest of that round still cooks (AC-12). A dish still cooking or
 * waiting on the pass can be cancelled with a reason (AC-13).
 *
 * **Opening this screen acknowledges its ready food**, so the reminder stops
 * chiming for a table the waiter is standing at (AC-10). The alert itself is
 * the waiter shell's, not this screen's, so it reaches the waiter wherever
 * they are.
 *
 * Any waiter may act on any table. The screen says who is responsible, the one
 * who hears its chime, and offers to take it over.
 */
export function WaiterTable() {
  const { visitId = '' } = useParams()
  const { t } = useTranslation(['waiter', 'common'])
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const me = useIdentity().staff.id

  const visit = useQuery(visitQuery(visitId))
  const menu = useQuery(menuQuery)
  const { basket, update, clear } = useSavedBasket(visitId)

  const { serving, serve } = useServeLine()
  const { taking, takeOver } = useTakeOver()
  const [servingRound, setServingRound] = useState<string | null>(null)
  const [voiding, setVoiding] = useState<OrderLine | null>(null)
  const [moving, setMoving] = useState(false)

  // The device's clock error, measured against the answer that just arrived.
  // Every age drawn below is corrected by it, so a phone with a wrong clock
  // still shows a true one.
  const offset = useMemo(() => (visit.data ? clockOffset(visit.data.serverTime) : 0), [visit.data])

  // Standing at the table is knowing its food is up.
  const readyHere = useMemo(
    () =>
      (visit.data?.rounds ?? []).flatMap((round) =>
        round.lines.filter((line) => line.status === 'ready').map((line) => line.id),
      ),
    [visit.data],
  )
  useEffect(() => {
    if (readyHere.length > 0) acknowledge(readyHere)
  }, [readyHere])

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['visit'] })

  const send = useMutation({
    mutationFn: (request: { visitId: string; clientKey: string; lines: SendLine[] }) =>
      sendRound(request.visitId, request.clientKey, request.lines),
    onSuccess: async (_round, request) => {
      // Cleared only once a send has landed, and for the visit it was sent
      // for, read from the request rather than from whatever this screen shows
      // by now. A send that failed any other way leaves the basket and its key
      // exactly as they were, so trying again is one tap and cannot make a
      // second ticket.
      clear(request.visitId)
      await refresh()
    },
    onError: (error: unknown, request) => {
      const code = failureCode(error)

      if (code === 'visit_not_open') {
        // Nothing can be sent to a table that has closed, so keeping the
        // basket would only invite another refusal.
        clear(request.visitId)
        showToast({ title: t('basket.discardedClosed'), tone: 'late' })
        void refresh()
        return
      }

      // A dish went off between the menu this screen holds and the send. The
      // whole ticket was refused, so nothing reached the kitchen; the menu is
      // read again, and the line that caused it is flagged in the basket.
      if (code === 'dish_not_orderable') {
        void queryClient.invalidateQueries({ queryKey: menuKey })
      }
      refuse(error)
    },
  })

  const serveRound = useMutation({
    mutationFn: (roundId: string) => markRoundServed(roundId),
    onSuccess: async () => {
      await refresh()
    },
    onError: (error: unknown) => {
      void refresh()
      refuse(error)
    },
    onSettled: () => {
      setServingRound(null)
    },
  })

  const close = useMutation({
    mutationFn: () => closeVisit(visitId),
    onSuccess: async () => {
      await refresh()
    },
    onError: (error: unknown) => {
      // The common refusal here is `bill_has_unserved_lines`, and the sentence
      // the waiter reads is translated from that code rather than rendered from
      // the API's English.
      void refresh()
      refuse(error)
    },
  })

  function refuse(error: unknown): void {
    showToast({
      title: apiErrorMessage(failureBody(error), common),
      tone: 'late',
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
  const mine = visit.data.responsibleStaffId === me
  const plates = basketSize(basket)
  // Checked against the menu this screen holds, which the live stream keeps
  // fresh: a dish the kitchen switches off is flagged here within a second or
  // two, before the waiter tries to send it.
  const flagged = unorderableDishes(basket, menu.data)
  const tooLong = basket.lines.some((line) => noteTooLong(line.note))

  /** Money on this bill, in the bill's own copied currency, never today's. */
  const billMoney = (amount: string): string =>
    bill ? formatMoney(amount, bill.currencyCode, bill.currencyDecimals) : amount

  function sendBasket(): void {
    // A basket saved before keys existed has none; it gets one now, and keeps
    // it, so a retry of this very send carries the same key.
    const clientKey = basket.clientKey ?? crypto.randomUUID()
    if (basket.clientKey === null) update((current) => ({ ...current, clientKey }))
    send.mutate({ visitId, clientKey, lines: sendLines(basket) })
  }

  return (
    <div className="space-y-6">
      <WaiterViews withMine={false} />
      <header className="space-y-2">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <h1 className="text-2xl font-semibold text-foreground">
            {t('table.title', { label: visit.data.tableLabel })}
          </h1>
          <p className="text-sm text-muted-foreground">
            {mine
              ? t('responsible.you')
              : t('responsible.named', { name: visit.data.responsibleName })}
          </p>
        </div>

        {!closed && (
          <div className="flex flex-wrap gap-2">
            {!mine && (
              <Button
                variant="secondary"
                size="sm"
                disabled={taking === visitId}
                onClick={() => {
                  takeOver(visitId, visit.data.responsibleStaffId)
                }}
              >
                {taking === visitId ? t('takeOver.taking') : t('takeOver.action')}
              </Button>
            )}
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                setMoving(true)
              }}
            >
              <Icon icon={ArrowRightLeft} size="sm" />
              {t('move.action')}
            </Button>
          </div>
        )}
      </header>

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
                <h3 className="mb-2 text-sm font-medium text-muted-foreground">
                  <RestaurantText>{category.name}</RestaurantText>
                </h3>
                <ul className="divide-y divide-border">
                  {category.dishes.map((dish) => {
                    const inBasket = basket.lines
                      .filter((line) => line.dishId === dish.id)
                      .reduce((total, line) => total + line.quantity, 0)

                    return (
                      <li key={dish.id} className="flex items-center justify-between gap-3 py-2">
                        <DietMark diet={dish.diet} />
                        <div className="min-w-0 flex-1">
                          <RestaurantText
                            as="p"
                            className={
                              dish.available
                                ? 'truncate text-card-foreground'
                                : 'truncate text-muted-foreground line-through'
                            }
                          >
                            {dish.name}
                          </RestaurantText>
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
                          {inBasket > 0 && (
                            <span className="tabular w-6 text-center text-sm">{inBasket}</span>
                          )}
                          <Button
                            size="sm"
                            disabled={!dish.available}
                            aria-label={t('table.addOne', { dish: dish.name })}
                            onClick={() => {
                              update((current) => addOne(current, dish.id, dish.name))
                            }}
                          >
                            +
                          </Button>
                        </div>
                      </li>
                    )
                  })}
                </ul>
              </Card>
            ))
          )}

          {plates > 0 && <BasketPanel basket={basket} onChange={update} flagged={flagged} />}

          <Button
            disabled={plates === 0 || flagged.length > 0 || tooLong || send.isPending}
            onClick={sendBasket}
          >
            {send.isPending ? t('table.sending') : t('table.send', { count: plates })}
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
              <RoundCard
                key={round.id}
                round={round}
                offset={offset}
                billMoney={billMoney}
                open={!closed}
                serving={serving}
                onServe={serve}
                servingRound={servingRound === round.id}
                onServeRound={() => {
                  setServingRound(round.id)
                  serveRound.mutate(round.id)
                }}
                onVoid={setVoiding}
              />
            ))}
          </ul>
        )}
      </section>

      {bill && (
        <Card>
          <h2 className="mb-2 flex items-center gap-2 text-lg font-medium text-card-foreground">
            <ReceiptText aria-hidden="true" className="size-5" />
            {bill.status === 'voided'
              ? t('bill.nothingTitle')
              : closed
                ? t('bill.closedTitle', { number: bill.number ?? 0 })
                : t('bill.runningTitle')}
          </h2>

          {bill.status === 'voided' ? (
            // Every dish was cancelled, or none was ordered: the bill was
            // voided rather than closed, so no number was used and there is
            // no total to show.
            <p className="text-sm text-muted-foreground">{t('bill.nothingBody')}</p>
          ) : (
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
          )}

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

      <VoidDialog
        line={voiding}
        onClose={() => {
          setVoiding(null)
        }}
      />
      <MoveDialog
        open={moving}
        onClose={() => {
          setMoving(false)
        }}
        visitId={visitId}
        tableLabel={visit.data.tableLabel}
      />
    </div>
  )
}

interface RoundCardProps {
  round: Round
  offset: number
  billMoney: (amount: string) => string
  /** Whether the table is still open, so its dishes can still be acted on. */
  open: boolean
  serving: string | null
  onServe: (lineId: string) => void
  servingRound: boolean
  onServeRound: () => void
  onVoid: (line: OrderLine) => void
}

/** One ticket: its status, its age, and every dish on it with what can be done. */
function RoundCard({
  round,
  offset,
  billMoney,
  open,
  serving,
  onServe,
  servingRound,
  onServeRound,
  onVoid,
}: RoundCardProps) {
  const { t } = useTranslation('waiter')
  const anyReady = round.lines.some((line) => line.status === 'ready')

  return (
    <Card as="li" className="space-y-2">
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
          <li key={line.id} className="space-y-2 py-2">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className={line.status === 'voided' ? 'text-muted-foreground line-through' : ''}>
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
                {line.status === 'voided' && line.voidReasonCode !== null && (
                  <p className="text-xs text-muted-foreground">
                    {t(`void.reason.${line.voidReasonCode}`)}
                    {line.voidReason !== null && (
                      <>
                        {' · '}
                        <RestaurantText>{line.voidReason}</RestaurantText>
                      </>
                    )}
                  </p>
                )}
              </div>
              <span className="flex items-center gap-3">
                <span className="tabular">{billMoney(line.lineTotal)}</span>
                <StatusPill status={line.status} compact />
              </span>
            </div>

            {open && (line.status === 'ready' || line.status === 'queued') && (
              <div className="flex flex-wrap justify-end gap-2">
                {line.status === 'ready' && (
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
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  aria-label={t('void.actionNamed', { dish: line.dishName })}
                  onClick={() => {
                    onVoid(line)
                  }}
                >
                  {t('void.action')}
                </Button>
              </div>
            )}
          </li>
        ))}
      </ul>

      {open && anyReady && (
        <Button disabled={servingRound} onClick={onServeRound}>
          {servingRound ? t('table.serving') : t('serve.allReady')}
        </Button>
      )}
    </Card>
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
