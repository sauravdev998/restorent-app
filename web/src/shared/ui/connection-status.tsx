import { Wifi, WifiOff } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import type { StreamStatus } from '@/shared/events/use-live-events'

import { cn } from './cn'
import { Icon } from './icon'

export interface ConnectionStatusProps {
  status: StreamStatus
  className?: string
}

/**
 * Whether the live stream is up, said in the header of every surface.
 *
 * Announced politely, so a screen reader mentions a dropped connection without
 * cutting across whatever is being read. A closed stream is the one that
 * matters: it means the browser has given up and will not retry, so staff need
 * to reload rather than wait.
 */
export function ConnectionStatus({ status, className }: ConnectionStatusProps) {
  const { t } = useTranslation()
  const down = status === 'closed'

  return (
    <span
      className={cn(
        'inline-flex items-center gap-2 text-xs',
        down ? 'text-status-late' : 'text-muted-foreground',
        className,
      )}
      aria-live="polite"
      data-testid="stream-status"
    >
      <Icon icon={down ? WifiOff : Wifi} size="sm" />
      {t(`stream.${status}`)}
    </span>
  )
}
