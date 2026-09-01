import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { Button } from './button'
import { cn } from './cn'
import { Icon } from './icon'
import { STATUS_PRESENTATION } from './status'
import { dismissToast, subscribeToToasts, type Toast as ToastItem } from './toast-store'

const DEFAULT_DURATION_MS = 6000

export interface ToastProps {
  toast: ToastItem
  className?: string
}

/**
 * One transient confirmation.
 *
 * It never takes focus, because stealing focus mid task is how a toast turns
 * into an interruption. It is dismissible from the keyboard through a real
 * button, and it announces through the shared live region rather than relying
 * on anyone seeing it.
 */
export function Toast({ toast, className }: ToastProps) {
  const { t } = useTranslation()
  const presentation = STATUS_PRESENTATION[toast.tone ?? 'ready']

  useEffect(() => {
    const timer = setTimeout(() => {
      dismissToast(toast.id)
    }, toast.durationMs ?? DEFAULT_DURATION_MS)

    return () => {
      clearTimeout(timer)
    }
  }, [toast.id, toast.durationMs])

  return (
    <li
      className={cn(
        'border-line flex items-start gap-3 rounded-lg border-border bg-card p-4 text-card-foreground',
        className,
      )}
    >
      <Icon icon={presentation.icon} size="md" className={presentation.text} />

      <div className="flex-1">
        <p className="text-sm font-medium">{toast.title}</p>
        {toast.description !== undefined && (
          <p className="mt-1 text-xs text-muted-foreground">{toast.description}</p>
        )}
      </div>

      <Button
        variant="ghost"
        size="sm"
        aria-label={t('toast.dismiss')}
        onClick={() => {
          dismissToast(toast.id)
        }}
      >
        {t('toast.dismiss')}
      </Button>
    </li>
  )
}

/**
 * Where toasts land, mounted once by `SurfaceShell`.
 *
 * Pinned to the block end and the inline end, which is the bottom right in
 * English and the bottom left in Arabic, without anything here knowing which.
 */
export function ToastViewport() {
  const { t } = useTranslation()
  const [toasts, setToasts] = useState<ToastItem[]>([])

  useEffect(() => subscribeToToasts(setToasts), [])

  if (toasts.length === 0) return null

  return (
    <ul
      aria-label={t('toast.region')}
      className="fixed end-4 bottom-4 z-50 flex w-96 max-w-[calc(100%-2rem)] flex-col gap-3"
    >
      {toasts.map((toast) => (
        <Toast key={toast.id} toast={toast} />
      ))}
    </ul>
  )
}
