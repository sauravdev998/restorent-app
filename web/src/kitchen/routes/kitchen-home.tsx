import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowUp, CookingPot, VolumeX } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useOutletContext } from 'react-router'

import { failureBody } from '@/shared/api/call-error'
import { apiErrorMessage } from '@/shared/api/error-message'
import { kitchenKey } from '@/shared/events/query-keys'
import { clockOffset, onDeviceClock } from '@/shared/events/server-clock'
import type { LiveEvents } from '@/shared/events/use-live-events'
import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/ui/cn'
import { EmptyState } from '@/shared/ui/empty-state'
import { Icon } from '@/shared/ui/icon'
import { useAudioUnlocked } from '@/shared/ui/audio-unlock'
import { Skeleton } from '@/shared/ui/skeleton'
import { showToast } from '@/shared/ui/toast-store'
import { useFreshIds } from '@/kitchen/alerts/use-fresh-ids'
import { useNewTickets } from '@/kitchen/alerts/use-new-tickets'
import {
  kitchenQuery,
  markLineReady,
  markRoundReady,
  unmarkLineReady,
  voidLineRanOut,
  type KitchenLine,
} from '@/kitchen/api/tickets'
import { KitchenTabs } from '@/kitchen/components/kitchen-tabs'
import { TicketCard, type PendingAct } from '@/kitchen/components/ticket-card'
import { useWakeLock } from '@/kitchen/use-wake-lock'

/**
 * The pass: every ticket the kitchen still has work on, and the four things a
 * wall mounted tablet needs to be honest about itself.
 *
 * **Two areas, one screen.** What is cooking comes first, oldest sent first,
 * because that is the order a kitchen works in. What is plated and waiting sits
 * below it, oldest plated first, and stays there until a waiter marks it served.
 * Until spec 0012 a ticket vanished at the moment it most needed watching, and
 * the only thing tracking uncollected food was a waiter's phone in an apron.
 *
 * **Every age is measured against the server's clock**, not this tablet's, and
 * against this restaurant's own two thresholds rather than a number written into
 * the screen. A kitchen tablet is a cheap appliance whose clock drifts and which
 * nobody ever looks at the time on, so the one number a chef acts on is corrected
 * by the difference the response itself reports.
 *
 * **A tap marks one dish and nothing else.** The ticket's own status follows from
 * its dishes and is recomputed by the server, which is why nothing here sets one,
 * and why the last dish makes the whole ticket ready by itself. Nothing is written
 * into the query cache ahead of the server: a dish is done when the pass says it
 * is, and a screen that greys it early is a screen that lies when the request
 * fails.
 *
 * **The screen says what it knows.** It says when it has stopped receiving, it
 * says when it cannot make a sound, it says when new work arrived above where the
 * chef is reading, and it says when there is more work than it is showing. A quiet
 * kitchen screen and a broken kitchen screen look identical otherwise.
 */
export function KitchenHome() {
  const { t } = useTranslation(['kitchen', 'common'])
  const { t: common } = useTranslation()
  const queryClient = useQueryClient()
  const live = useOutletContext<LiveEvents>()

  const queue = useQuery(kitchenQuery)

  // Held awake while this screen is open, released when the chef walks to the
  // Menu tab or signs out.
  useWakeLock()

  // Watched rather than read once while rendering. The prompt below has to go
  // on the very tap that opens the audio context, and nothing else redraws this
  // screen for it: a kitchen tablet can sit untouched between tickets, so a
  // prompt waiting for the next redraw is a prompt that stays up for the shift.
  const audioUnlocked = useAudioUnlocked()

  // Which dish is mid tap, so its own button says so and no second tap lands on
  // the same dish while the first is in flight.
  const [pending, setPending] = useState<PendingAct | null>(null)
  const [clearing, setClearing] = useState<string | null>(null)

  const offset = useMemo(() => (queue.data ? clockOffset(queue.data.serverTime) : 0), [queue.data])

  const ticketIds = useMemo(
    () => queue.data?.tickets.map((ticket) => ticket.id) ?? null,
    [queue.data],
  )
  const newTickets = useNewTickets(ticketIds)

  const cancelledIds = useMemo(
    () =>
      queue.data?.tickets.flatMap((ticket) =>
        ticket.lines.filter((line) => line.status === 'voided').map((line) => line.id),
      ) ?? null,
    [queue.data],
  )
  const freshCancelIds = useFreshIds(cancelledIds)

  // One shape for all four acts: ask, then refetch, whether it worked or not. A
  // refusal is almost always somebody else having got there first, and the only
  // honest answer to that is to show where the ticket really is.
  const refetch = async (): Promise<void> => {
    await queryClient.invalidateQueries({ queryKey: kitchenKey })
  }

  const complain = (error: unknown): void => {
    void refetch()
    showToast({ title: apiErrorMessage(failureBody(error), common), tone: 'late' })
  }

  const act = useMutation({
    mutationFn: (request: PendingAct): Promise<void> => {
      if (request.act === 'ready') return markLineReady(request.lineId)
      if (request.act === 'unready') return unmarkLineReady(request.lineId)
      return voidLineRanOut(request.lineId)
    },
    onSuccess: refetch,
    onError: complain,
    onSettled: () => {
      setPending(null)
    },
  })

  const clearTicket = useMutation({
    mutationFn: (roundId: string) => markRoundReady(roundId),
    onSuccess: refetch,
    onError: complain,
    onSettled: () => {
      setClearing(null)
    },
  })

  const run = (request: PendingAct): void => {
    setPending(request)
    act.mutate(request)
  }

  if (queue.isPending) {
    return (
      <PassFrame>
        <Skeleton className="h-72 w-full" label={common('loading.label')} />
      </PassFrame>
    )
  }

  if (queue.isError) {
    return (
      <PassFrame>
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
      </PassFrame>
    )
  }

  const { tickets, warningAfterSeconds, lateAfterSeconds, truncatedCount } = queue.data
  const cooking = tickets.filter((ticket) => ticket.status !== 'ready')
  const plated = tickets.filter((ticket) => ticket.status === 'ready')

  // Dimmed, never disabled. Every act on this screen is an ordinary request and
  // works perfectly well with the stream down; what is lost is being told about
  // somebody else's change, which the refetch on every reconnect catches up. A
  // screen that greyed its buttons out would turn a degraded shift into a stopped
  // one.
  const notLive = live.status === 'closed'

  const cardProps = (ticket: (typeof tickets)[number]) => ({
    ticket,
    since: onDeviceClock(
      ticket.status === 'ready' && ticket.readyAt !== null && ticket.readyAt !== undefined
        ? ticket.readyAt
        : ticket.sentAt,
      offset,
    ),
    warningAfterSeconds,
    lateAfterSeconds,
    isNew: newTickets.ids.has(ticket.id),
    freshCancelIds,
    pending,
    clearing: clearing === ticket.id,
    onReady: (lineId: string) => {
      run({ lineId, act: 'ready' })
    },
    onUnready: (lineId: string) => {
      run({ lineId, act: 'unready' })
    },
    onRanOut: (line: KitchenLine) => {
      run({ lineId: line.id, act: 'void' })
    },
    onAllDone: (roundId: string) => {
      setClearing(roundId)
      clearTicket.mutate(roundId)
    },
  })

  return (
    <PassFrame>
      {/* Its own band on the pass, beside the shell's. The shell's says live
          updates have stopped; this one says what that means for this screen and
          dims the work below it, because a chef reading a wall from three metres
          away needs the whole surface to look wrong, not a line of text. */}
      {notLive && (
        <div
          role="status"
          className="border-line w-full border-status-late bg-card p-4 text-status-late"
          data-testid="pass-not-live"
        >
          <p className="text-lg font-semibold">{t('pass.notLiveTitle')}</p>
          <p className="text-sm text-card-foreground">{t('pass.notLiveBody')}</p>
        </div>
      )}

      {/* Audio is locked until somebody touches the page, and a kitchen tablet can
          sit untouched for an hour. The prompt makes the silence visible and
          fixable. It does not make the sound reliable, and nothing on this screen
          depends on it. */}
      {!audioUnlocked && (
        <div
          className="border-line flex items-center gap-3 border-border bg-card p-4"
          data-testid="pass-sound-off"
        >
          <Icon icon={VolumeX} size="lg" className="text-muted-foreground" />
          <div>
            <p className="text-lg font-semibold text-card-foreground">{t('pass.soundOffTitle')}</p>
            <p className="text-sm text-muted-foreground">{t('pass.soundOffBody')}</p>
          </div>
        </div>
      )}

      {/* New work arrived while the chef was reading further down. The scroll is
          left exactly where their hand is; this offers the way back rather than
          taking it. */}
      {newTickets.aboveTheFold && (
        <div
          role="status"
          className="border-line sticky top-0 z-10 flex items-center justify-between gap-3 border-primary bg-card p-3 text-primary"
          data-testid="new-work-above"
        >
          <span className="flex items-center gap-2 text-base font-semibold">
            <Icon icon={ArrowUp} size="md" />
            {t('pass.newWorkAbove')}
          </span>
          <Button variant="secondary" onClick={newTickets.goToTop}>
            {t('pass.backToTop')}
          </Button>
        </div>
      )}

      {truncatedCount > 0 && (
        <p
          role="status"
          className="border-line border-status-late bg-card p-3 text-sm text-status-late"
          data-testid="pass-truncated"
        >
          {t('pass.hidden', { count: truncatedCount })}
        </p>
      )}

      <div className={cn('space-y-8', notLive && 'opacity-60')} data-dimmed={notLive || undefined}>
        <section aria-labelledby="cooking-heading" className="space-y-4">
          <h2 id="cooking-heading" className="text-xl font-semibold text-foreground">
            {t('pass.cookingHeading')}
          </h2>

          {cooking.length === 0 ? (
            <EmptyState
              icon={CookingPot}
              title={t('pass.quietTitle')}
              description={t('pass.quietBody')}
            />
          ) : (
            // As many columns as fit a card at least 28rem wide, rather than a
            // column count per breakpoint. Breakpoints ride the viewport, and
            // at kitchen type size three columns on a laptop squeezed each
            // dish into a sliver beside its buttons. Both lists share it.
            <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,28rem),1fr))] gap-4">
              {cooking.map((ticket) => (
                <TicketCard key={ticket.id} {...cardProps(ticket)} />
              ))}
            </ul>
          )}
        </section>

        <section aria-labelledby="ready-heading" className="space-y-4">
          <h2 id="ready-heading" className="text-xl font-semibold text-foreground">
            {t('pass.readyHeading')}
          </h2>

          {plated.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('pass.readyEmpty')}</p>
          ) : (
            <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,28rem),1fr))] gap-4">
              {plated.map((ticket) => (
                <TicketCard key={ticket.id} {...cardProps(ticket)} />
              ))}
            </ul>
          )}
        </section>
      </div>
    </PassFrame>
  )
}

/** The tabs and the heading, which every state of this screen carries. */
function PassFrame({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation('kitchen')

  return (
    <div className="space-y-4">
      <KitchenTabs />
      <h1 className="text-2xl font-semibold text-foreground">{t('pass.title')}</h1>
      {children}
    </div>
  )
}
