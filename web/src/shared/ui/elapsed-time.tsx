import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from './cn'
import { Icon } from './icon'
import { STATUS_PRESENTATION } from './status'

/**
 * How long a round may wait before it reads as late.
 *
 * A placeholder, and it is written down as one. The real threshold belongs to
 * each restaurant and is feature 13's decision. `ElapsedTime` takes it as a
 * prop so feature 13 supplies the real value without anything here being
 * rewritten.
 */
export const DEFAULT_LATE_AFTER_SECONDS = 900

export interface ElapsedTimeProps {
  /** The `timestamptz` the clock counts from, e.g. a round's `created_at`. */
  since: string
  /** Seconds after which this reads as late. Defaults to the placeholder above. */
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
 * spelling out a colon.
 *
 * Deliberately not `aria-label`. `<time>` carries no ARIA role, and an
 * `aria-label` on a role less element is simply ignored by screen readers, so
 * the label would have looked correct in the source and announced nothing. Real
 * hidden text always works.
 */
export function ElapsedTime({
  since,
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
  const late = total >= lateAfterSeconds

  const clock = `${String(minutes)}:${String(seconds).padStart(2, '0')}`
  const spoken = `${t('elapsed.minutes', { count: minutes })}, ${t('elapsed.seconds', {
    count: seconds,
  })}`

  return (
    <time
      dateTime={`PT${String(minutes)}M${String(seconds)}S`}
      className={cn(
        'tabular inline-flex items-center gap-2 text-sm',
        late && STATUS_PRESENTATION.late.text,
        className,
      )}
      data-late={late || undefined}
    >
      {late && <Icon icon={STATUS_PRESENTATION.late.icon} size="sm" />}
      <span aria-hidden="true">{clock}</span>
      <span className="sr-only">{late ? `${spoken}, ${t('status.late')}` : spoken}</span>
    </time>
  )
}
