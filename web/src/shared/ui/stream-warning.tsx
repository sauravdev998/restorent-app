import { WifiOff } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import type { StreamStatus } from '@/shared/events/use-live-events'

import { announce } from './announce'
import { cn } from './cn'
import { Icon } from './icon'

export interface StreamWarningProps {
  status: StreamStatus
  className?: string
}

/**
 * A band across the top of the screen saying that live updates have stopped.
 *
 * The `ConnectionStatus` chip in the header says the same thing in three
 * characters, and that is the right size for it while everything is fine. It is
 * the wrong size for the one case that matters: a kitchen screen that has
 * quietly stopped receiving tickets looks exactly like a kitchen with no
 * orders, and a chef reading it from three metres away will not notice a small
 * grey word in the corner. So a stopped stream gets a band, and the band says
 * what to do about it.
 *
 * **Nothing here disables anything.** Every action on both screens still works
 * with the stream down: the requests are ordinary HTTP and have nothing to do
 * with it. What is lost is being told about somebody else's change, and the
 * refetch that runs on every reconnect catches all of that up. A screen that
 * greyed its buttons out would turn a degraded shift into a stopped one.
 *
 * Announced politely and exactly once per drop. Assertive would cut across
 * whatever a screen reader is in the middle of, and a connection that has been
 * down for ten minutes is not more urgent on the tenth minute than the first.
 */
export function StreamWarning({ status, className }: StreamWarningProps) {
  const { t } = useTranslation()
  const announced = useRef(false)

  const down = status === 'closed'

  useEffect(() => {
    if (!down) {
      // Reset on recovery, so the next drop announces again. Without this the
      // one announcement per page load would be the only one a person ever got.
      announced.current = false
      return
    }

    if (announced.current) return
    announced.current = true

    announce(`${t('connection.lostTitle')}. ${t('connection.lostBody')}`, 'polite')
  }, [down, t])

  if (!down) return null

  return (
    <div
      role="status"
      className={cn(
        'border-line flex items-start gap-3 border-status-late bg-card px-4 py-3 text-status-late',
        className,
      )}
      data-testid="stream-warning"
    >
      <Icon icon={WifiOff} size="md" />
      <div>
        <p className="font-semibold">{t('connection.lostTitle')}</p>
        <p className="text-sm text-card-foreground">{t('connection.lostBody')}</p>
      </div>
    </div>
  )
}
