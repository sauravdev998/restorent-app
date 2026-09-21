import { useTranslation } from 'react-i18next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { cn } from '@/shared/ui/cn'
import { ElapsedTime } from '@/shared/ui/elapsed-time'
import { RestaurantText } from '@/shared/ui/restaurant-text'
import { StatusPill } from '@/shared/ui/status-pill'
import type { KitchenLine, KitchenTicket } from '@/kitchen/api/tickets'

/** Which of a ticket's three per dish actions is mid request, and on which dish. */
export interface PendingAct {
  lineId: string
  act: 'ready' | 'unready' | 'void'
}

export interface TicketCardProps {
  ticket: KitchenTicket
  /** The timestamp to count the age from, already shifted onto this device's clock. */
  since: string
  /** This restaurant's amber threshold, in seconds. */
  warningAfterSeconds: number
  /** This restaurant's red threshold, in seconds. */
  lateAfterSeconds: number
  /** Whether this ticket has not been on this screen before. */
  isNew: boolean
  /** The dishes cancelled since this screen last drew, which flash once. */
  freshCancelIds: ReadonlySet<string>
  /** The one act in flight anywhere on the pass, or `null`. */
  pending: PendingAct | null
  /** Whether All done is mid request for this ticket. */
  clearing: boolean
  onReady: (lineId: string) => void
  onUnready: (lineId: string) => void
  onRanOut: (line: KitchenLine) => void
  onAllDone: (roundId: string) => void
}

/**
 * One kitchen ticket: where the food is going, how long it has waited, what to
 * cook, and what was cancelled.
 *
 * **Three groups of dishes, in the order a chef needs them.** What is still
 * cooking comes first and carries the one action worth a full sized button. What
 * is plated sits under it with the undo beside it, because a chef who taps the
 * wrong dish notices immediately and the fix has to be right there. What was
 * cancelled goes to a clearly separated strip at the bottom, with its reason, so
 * a dish never simply vanishes mid cook.
 *
 * **Nothing here decides anything.** The ticket's own status, when a dish may be
 * undone, and when the whole ticket flips are all the server's answers. This draws
 * them.
 *
 * There is not one kitchen specific size class on it, and there should never be.
 * `data-surface="kitchen"` on the document is what makes the same `Button` 72
 * pixels tall here and 36 for an admin.
 */
export function TicketCard({
  ticket,
  since,
  warningAfterSeconds,
  lateAfterSeconds,
  isNew,
  freshCancelIds,
  pending,
  clearing,
  onReady,
  onUnready,
  onRanOut,
  onAllDone,
}: TicketCardProps) {
  const { t } = useTranslation('kitchen')
  const { t: common } = useTranslation()

  const cooking = ticket.lines.filter((line) => line.status === 'queued')
  const plated = ticket.lines.filter((line) => line.status === 'ready')
  const cancelled = ticket.lines.filter((line) => line.status === 'voided')
  const served = ticket.lines.filter((line) => line.status === 'served')

  const busy = (lineId: string, act: PendingAct['act']): boolean =>
    pending?.lineId === lineId && pending.act === act

  return (
    <Card
      as="li"
      className={cn('space-y-3', isNew && 'border-primary')}
      data-testid="kitchen-ticket"
      data-new={isNew || undefined}
    >
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <span className="text-xl font-semibold text-card-foreground">
          {t('pass.table', { label: ticket.tableLabel })}
        </span>
        <span className="flex items-center gap-3 text-sm text-muted-foreground">
          {isNew && (
            <span className="rounded-full border-2 border-primary px-3 py-1 font-semibold text-primary">
              {t('pass.newTicket')}
            </span>
          )}
          {t('pass.round', { number: ticket.sequenceNo })}
        </span>
      </header>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">
            {ticket.status === 'ready' ? t('pass.platedSince') : t('pass.waitingSince')}
          </span>
          <ElapsedTime
            since={since}
            warningAfterSeconds={warningAfterSeconds}
            lateAfterSeconds={lateAfterSeconds}
          />
        </span>
        <StatusPill status={ticket.status} />
      </div>

      {cooking.length > 0 && (
        <>
          {/* One tap for the whole ticket, beside the dishes it clears rather
              than in the header, so it reads as "all of these" instead of as
              something that might do more. */}
          <div className="flex justify-end">
            <Button
              variant="secondary"
              disabled={clearing}
              aria-label={t('pass.allDoneLabel', { label: ticket.tableLabel })}
              onClick={() => {
                onAllDone(ticket.id)
              }}
            >
              {clearing ? t('pass.allDoneBusy') : t('pass.allDone')}
            </Button>
          </div>

          <ul className="space-y-2">
            {cooking.map((line) => (
              <li key={line.id} className="flex items-start justify-between gap-3">
                <DishText line={line} />
                <div className="flex shrink-0 flex-col items-end gap-2">
                  <Button
                    disabled={busy(line.id, 'ready')}
                    onClick={() => {
                      onReady(line.id)
                    }}
                  >
                    {busy(line.id, 'ready') ? t('pass.marking') : t('pass.done')}
                  </Button>
                  {/* The kitchen's own void, and the only reason a chef may
                      give: we have run out. Destructive, because it takes the
                      dish off the guest's bill. */}
                  <Button
                    variant="destructive"
                    size="sm"
                    disabled={busy(line.id, 'void')}
                    aria-label={t('pass.ranOutLabel', { dish: line.dishName })}
                    onClick={() => {
                      onRanOut(line)
                    }}
                  >
                    {busy(line.id, 'void') ? t('pass.ranOutBusy') : t('pass.ranOut')}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}

      {plated.length > 0 && (
        <ul className="space-y-2">
          {plated.map((line) => (
            <li key={line.id} className="flex items-start justify-between gap-3">
              <DishText line={line} muted />
              <div className="flex shrink-0 items-center gap-2">
                <StatusPill status="ready" compact />
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={busy(line.id, 'unready')}
                  aria-label={t('pass.undoLabel', { dish: line.dishName })}
                  onClick={() => {
                    onUnready(line.id)
                  }}
                >
                  {busy(line.id, 'unready') ? t('pass.undoBusy') : t('pass.undo')}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {served.length > 0 && (
        <ul className="space-y-2">
          {served.map((line) => (
            <li key={line.id} className="flex items-start justify-between gap-3">
              <DishText line={line} muted />
              <StatusPill status="served" />
            </li>
          ))}
        </ul>
      )}

      {cancelled.length > 0 && (
        <section
          // Its own bordered strip at the foot of the card, not a struck through
          // line in the cook list. A chef scanning for what to cook should be
          // able to skip this whole block with their eyes, and a dish that
          // disappeared mid cook without explanation is how a guest gets nothing.
          className="border-t-line space-y-2 border-status-voided pt-3"
          aria-label={t('pass.cancelledHeading')}
          data-testid="cancelled-strip"
        >
          <h3 className="text-xs font-semibold tracking-wide text-status-voided uppercase">
            {t('pass.cancelledHeading')}
          </h3>
          {cancelled.map((line) => (
            <div
              key={line.id}
              // Flashes once, at the moment it is cancelled and never again, so a
              // chef mid cook notices a dish leaving their list. Once, not a
              // pulse: a kitchen screen that keeps moving is a kitchen screen
              // nobody reads. The animation drops to nothing under reduced
              // motion, where the strip itself still carries the meaning.
              className={cn('space-y-1', freshCancelIds.has(line.id) && 'flash-once')}
              data-fresh={freshCancelIds.has(line.id) || undefined}
            >
              <p className="font-medium text-status-voided line-through">
                {line.quantity} × <RestaurantText>{line.dishName}</RestaurantText>
              </p>
              <p className="text-xs text-muted-foreground">
                {t('pass.cancelledReason', {
                  reason:
                    line.voidReasonCode === null || line.voidReasonCode === undefined
                      ? common('status.voided')
                      : common(`voidReason.${line.voidReasonCode}`),
                })}
              </p>
              {/* Only `other` carries words, and only because the code says
                  nothing on its own. */}
              {line.voidReason !== null && line.voidReason !== undefined && (
                <RestaurantText
                  as="p"
                  className="text-xs break-words whitespace-pre-wrap text-muted-foreground"
                >
                  {line.voidReason}
                </RestaurantText>
              )}
            </div>
          ))}
        </section>
      )}
    </Card>
  )
}

/** A dish's quantity, name, and the guest's own words about it. */
function DishText({ line, muted = false }: { line: KitchenLine; muted?: boolean }) {
  return (
    <div className="min-w-0">
      <p className={cn('font-medium', muted ? 'text-muted-foreground' : 'text-card-foreground')}>
        {line.quantity} × <RestaurantText>{line.dishName}</RestaurantText>
      </p>
      {/* Reaches the pass exactly as the guest said it, whole and wrapped over
          as many lines as it needs, never cut off: "no onions, nut allergy"
          truncated to "no onions, nu" is a hospital visit. */}
      {line.note !== null && line.note !== undefined && line.note !== '' && (
        <RestaurantText
          as="p"
          className="text-sm break-words whitespace-pre-wrap text-muted-foreground"
        >
          {line.note}
        </RestaurantText>
      )}
    </div>
  )
}
