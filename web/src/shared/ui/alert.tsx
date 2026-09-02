import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'

import { playReadyChime } from './audio-unlock'
import { Button } from './button'
import { cn } from './cn'
import { Icon } from './icon'
import { announce } from './announce'
import { STATUS_PRESENTATION, type StatusTone } from './status'

export interface AlertProps {
  /** Shown and announced while true. Flipping it to true is what fires the alert. */
  open: boolean
  title: string
  description?: string
  /** Which status this is about. Decides the colour and the icon. */
  tone?: StatusTone
  onDismiss?: () => void
  /** Try a chime as well. Defaults to true, and never replaces the other two channels. */
  sound?: boolean
  className?: string
}

/**
 * The one that has to get through: food is ready.
 *
 * Three channels, in this order and never any other. The badge appears, the
 * announcement goes out through the shared live region, and only then is a
 * chime attempted. A phone on silent, a phone whose audio was never unlocked
 * because nobody has tapped the screen yet, and a deaf waiter all get the
 * alert, because the sound was never the alert in the first place.
 */
export function Alert({
  open,
  title,
  description,
  tone = 'ready',
  onDismiss,
  sound = true,
  className,
}: AlertProps) {
  const { t } = useTranslation()
  const presentation = STATUS_PRESENTATION[tone]

  useEffect(() => {
    if (!open) return

    // Assertive, because a plate under a heat lamp does not wait politely for
    // the screen reader to finish the sentence it was on.
    announce(description === undefined ? title : `${title}. ${description}`, 'assertive')

    if (sound) playReadyChime()
  }, [open, title, description, sound])

  if (!open) return null

  return (
    <div
      className={cn(
        'border-line flex items-start gap-4 rounded-lg border-current bg-card p-4',
        presentation.text,
        className,
      )}
      data-tone={tone}
    >
      <Icon icon={presentation.icon} size="lg" />

      <div className="flex-1">
        <p className="text-base font-semibold">{title}</p>
        {description !== undefined && (
          <p className="mt-1 text-sm text-card-foreground">{description}</p>
        )}
      </div>

      {onDismiss && (
        <Button variant="ghost" size="sm" onClick={onDismiss}>
          {t('toast.dismiss')}
        </Button>
      )}
    </div>
  )
}
