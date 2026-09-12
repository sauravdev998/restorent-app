import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { CookingPot } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { kitchenKey } from '@/shared/events/query-keys'
import { clockOffset, onDeviceClock } from '@/shared/events/server-clock'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ElapsedTime } from '@/shared/ui/elapsed-time'
import { EmptyState } from '@/shared/ui/empty-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { StatusPill } from '@/shared/ui/status-pill'
import { showToast } from '@/shared/ui/toast-store'
import { kitchenQuery, markLineReady } from '@/kitchen/api/tickets'
import { KitchenTabs } from '@/kitchen/components/kitchen-tabs'

/**
 * The pass: every ticket the kitchen still has work on, oldest first.
 *
 * There is not one kitchen specific size class on this screen and there should
 * never be. `RootLayout` puts `data-surface="kitchen"` on the document, which
 * makes `--spacing` bigger, and the same `Button` that is 36 pixels tall for an
 * admin is 72 here. That is what makes it readable across a room and hittable
 * with a gloved hand.
 *
 * **Every age is measured against the server's clock**, not this tablet's. A
 * kitchen screen is a cheap appliance whose clock drifts and which nobody ever
 * looks at the time on, so the one number a chef acts on is corrected by the
 * difference the response itself reports.
 *
 * **A tap marks one dish and nothing else.** The ticket's own status follows
 * from its dishes and is recomputed by the server, which is why nothing here
 * sets one, and why the last dish makes the whole ticket ready by itself.
 *
 * Feature 13 builds the real kitchen display over this.
 */
export function KitchenHome() {
  const { t } = useTranslation(['kitchen', 'common'])
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()

  const queue = useQuery(kitchenQuery)

  // Which dish is mid tap, so its own button says so. Nothing is written into
  // the cache ahead of the server: a dish is done when the pass says it is, and
  // a screen that greys it early is a screen that lies when the request fails.
  const [pending, setPending] = useState<string | null>(null)

  const offset = useMemo(() => (queue.data ? clockOffset(queue.data.serverTime) : 0), [queue.data])

  const mark = useMutation({
    mutationFn: (lineId: string) => markLineReady(lineId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: kitchenKey })
    },
    onError: (error: unknown) => {
      // Almost always `line_not_queued`: another chef got there first. The
      // queue is refetched so the screen shows where the ticket really is
      // rather than the state the tap assumed.
      void queryClient.invalidateQueries({ queryKey: kitchenKey })

      showToast({
        title: apiErrorMessage(failureBody(error), common),
        tone: 'late',
      })
    },
    onSettled: () => {
      setPending(null)
    },
  })

  if (queue.isPending) {
    return (
      <div className="space-y-4">
        <KitchenTabs />
        <h1 className="text-2xl font-semibold text-foreground">{t('pass.title')}</h1>
        <Skeleton className="h-72 w-full" label={common('loading.label')} />
      </div>
    )
  }

  if (queue.isError) {
    return (
      <div className="space-y-4">
        <KitchenTabs />
        <h1 className="text-2xl font-semibold text-foreground">{t('pass.title')}</h1>
        <EmptyState
          icon={CookingPot}
          title={common('error.title')}
          description={apiErrorMessage(failureBody(queue.error), common)}
          action={
            <Button
              onClick={() => {
                void queue.refetch()
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
      <KitchenTabs />
      <h1 className="text-2xl font-semibold text-foreground">{t('pass.title')}</h1>

      {queue.data.tickets.length === 0 ? (
        <EmptyState
          icon={CookingPot}
          title={t('pass.quietTitle')}
          description={t('pass.quietBody')}
        />
      ) : (
        <ul className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {queue.data.tickets.map((ticket) => (
            <Card as="li" key={ticket.id} className="space-y-3">
              <header className="flex flex-wrap items-baseline justify-between gap-2">
                <span className="text-xl font-semibold text-card-foreground">
                  {t('pass.table', { label: ticket.tableLabel })}
                </span>
                <span className="text-sm text-muted-foreground">
                  {t('pass.round', { number: ticket.sequenceNo })}
                </span>
              </header>

              <div className="flex items-center justify-between gap-3">
                <ElapsedTime since={onDeviceClock(ticket.sentAt, offset)} />
                <StatusPill status={ticket.status} />
              </div>

              <ul className="space-y-2">
                {ticket.lines.map((line) => (
                  <li key={line.id} className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <p className="truncate font-medium text-card-foreground">
                        {line.quantity} × {line.dishName}
                      </p>
                      {/* Reaches the pass exactly as the guest said it. */}
                      {line.note !== null && line.note !== '' && (
                        <p className="truncate text-sm text-muted-foreground">{line.note}</p>
                      )}
                    </div>

                    {line.status === 'queued' ? (
                      <Button
                        disabled={pending === line.id}
                        onClick={() => {
                          setPending(line.id)
                          mark.mutate(line.id)
                        }}
                      >
                        {pending === line.id ? t('pass.marking') : t('pass.done')}
                      </Button>
                    ) : (
                      <StatusPill status={line.status} />
                    )}
                  </li>
                ))}
              </ul>
            </Card>
          ))}
        </ul>
      )}
    </div>
  )
}
