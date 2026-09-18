import { useTranslation } from 'react-i18next'

import { cn } from '@/shared/ui/cn'
import { Icon } from '@/shared/ui/icon'
import { STATUS_PRESENTATION } from '@/shared/ui/status'

export interface ReadyBadgeProps {
  /** How many dishes on the table are waiting to be carried out. */
  count: number
  className?: string
}

/**
 * How many dishes on a table are ready to carry out, as a badge (spec 0011,
 * AC-1, AC-2).
 *
 * Every waiter sees it, whoever is responsible for the table, and it makes no
 * sound: only the responsible waiter's phone chimes. Nothing is drawn when
 * nothing is ready. Icon, number, and word together, so it never rests on
 * colour.
 */
export function ReadyBadge({ count, className }: ReadyBadgeProps) {
  const { t } = useTranslation('waiter')
  const presentation = STATUS_PRESENTATION.ready

  if (count === 0) return null

  return (
    <span
      className={cn(
        'border-line inline-flex items-center gap-2 rounded-full px-3 py-1 text-xs font-semibold',
        presentation.text,
        presentation.border,
        className,
      )}
      data-status="ready"
    >
      <Icon icon={presentation.icon} size="sm" />
      {t('readyBadge', { count })}
    </span>
  )
}
