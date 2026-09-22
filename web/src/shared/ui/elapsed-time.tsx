import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { formatUnitList } from '@/shared/format'

import { cn } from './cn'
import { Icon } from './icon'
import { STATUS_PRESENTATION } from './status'
import { urgencyOf } from './urgency'

/**
 * How long a round may wait before it reads as late, when nobody says.
 *
 * A fallback, and only that. The real thresholds belong to each restaurant, are
 * stored on its row, and reach every screen that draws an age through the read
 * that carries the age itself (spec 0012). Nothing in the product relies on this
 * number: it is here so a component rendered in a test or in the `/design`
 * gallery, with no restaurant behind it, still has something to compare against.
 */
export const DEFAULT_LATE_AFTER_SECONDS = 900

export interface ElapsedTimeProps {
  /** The `timestamptz` the clock counts from, e.g. a round's `sent_at`. */
  since: string
  /**
   * Seconds after which this reads amber. Absent means it never does, which is
   * what every screen outside the kitchen wants: one threshold, one colour.
   */
  warningAfterSeconds?: number
  /** Seconds after which this reads red. Defaults to the fallback above. */
  lateAfterSeconds?: number
  className?: string
}

function secondsSince(iso: string, now: number): number {
  const started = Date.parse(iso)
  if (Number.isNaN(started)) return 0
  return Math.max(0, Math.floor((now - started) / 1000))
}

/**
 * A duration that ticks, in the mono face with fixed width digits.
 *
 * The digits are tabular so a ticket that has been waiting 9 minutes and one
 * that has been waiting 10 do not sit at different widths in the same column,
 * which is what makes a wall of tickets scannable rather than jittery.
 *
 * The visible text is short because a chef reads it from three metres away. The
 * whole duration in words sits beside it, hidden from sight but not from a
 * screen reader, so it says "twelve minutes, thirty seconds" rather than
 * spelling out a colon. The words and the separator between them both come from
 * the active language rather than being assembled in English here.
 *
 * **Two thresholds, and colour is never the only difference between them.**
 * Amber and red each bring their own icon and their own hidden word, so a
 * greyscale print out, a colour blind chef, and a screen in forced colours mode
 * all still read three distinct states rather than one.
 *
 * Deliberately not `aria-label`. `<time>` carries no ARIA role, and an
 * `aria-label` on a role less element is simply ignored by screen readers, so
 * the label would have looked correct in the source and announced nothing. Real
 * hidden text always works.
 */
export function ElapsedTime({
  since,
  warningAfterSeconds,
  lateAfterSeconds = DEFAULT_LATE_AFTER_SECONDS,
  className,
}: ElapsedTimeProps) {
  const { t } = useTranslation()
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const timer = setInterval(() => {
      setNow(Date.now())
    }, 1000)
    return () => {
      clearInterval(timer)
    }
  }, [])

  const total = secondsSince(since, now)
  const minutes = Math.floor(total / 60)
  const seconds = total % 60
  const urgency = urgencyOf(total, warningAfterSeconds, lateAfterSeconds)

  const clock = `${String(minutes)}:${String(seconds).padStart(2, '0')}`

  // Joined by `Intl.ListFormat`, not by a comma written here. A comma is
  // English punctuation, and the separator between the parts of a duration is
  // not the same mark in every language. The plural forms come from the
  // language's own rules for the same reason.
  const spoken = formatUnitList([
    t('elapsed.minutes', { count: minutes }),
    t('elapsed.seconds', { count: seconds }),
  ])

  // Amber borrows the queued tone rather than inventing a sixth: this is the
  // cooking colour saying "still cooking, and it has been a while". Red is the
  // one derived emphasis the design system already has.
  const tone =
    urgency === 'late'
      ? STATUS_PRESENTATION.late
      : urgency === 'warning'
        ? STATUS_PRESENTATION.queued
        : undefined

  return (
    <time
      dateTime={`PT${String(minutes)}M${String(seconds)}S`}
      className={cn('tabular inline-flex items-center gap-2 text-sm', tone?.text, className)}
      data-late={urgency === 'late' || undefined}
      data-urgency={urgency}
    >
      {tone && <Icon icon={tone.icon} size="sm" />}
      <span aria-hidden="true">{clock}</span>
      <span className="sr-only">
        {urgency === 'late'
          ? formatUnitList([spoken, t('status.late')])
          : urgency === 'warning'
            ? formatUnitList([spoken, t('elapsed.gettingLate')])
            : spoken}
      </span>
    </time>
  )
}
