import { useTranslation } from 'react-i18next'

import { cn } from './cn'
import { Icon } from './icon'
import { STATUS_PRESENTATION, type StatusTone } from './status'

export interface StatusPillProps {
  status: StatusTone
  /** Hides the word and leaves the icon, for a very tight row. Rare, and it still announces. */
  compact?: boolean
  className?: string
}

/**
 * A dish's state, said three ways at once.
 *
 * Colour, a translated word, and an icon, always all three. Print the screen in
 * greyscale, hand it to a chef with red green colour blindness, or run it in
 * the operating system's high contrast mode, and every status is still
 * distinguishable, because two of the three signals do not depend on colour at
 * all.
 *
 * The background stays transparent on purpose: that keeps the measured contrast
 * pair "status colour on the surface behind it", which is a pair the contrast
 * gate actually checks, rather than a tinted fill nobody verified.
 */
export function StatusPill({ status, compact = false, className }: StatusPillProps) {
  const { t } = useTranslation()
  const presentation = STATUS_PRESENTATION[status]
  const word = t(presentation.labelKey)

  return (
    <span
      className={cn(
        'border-line inline-flex items-center gap-2 rounded-full px-3 py-1 text-xs font-medium',
        presentation.text,
        presentation.border,
        presentation.dashed && 'border-dashed',
        className,
      )}
      data-status={status}
    >
      <Icon icon={presentation.icon} size="sm" />
      {compact ? <span className="sr-only">{word}</span> : word}
    </span>
  )
}
