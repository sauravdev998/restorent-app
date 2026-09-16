import * as RadixDialog from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { useEffect, useRef, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { Button } from './button'
import { cn } from './cn'
import { Icon } from './icon'

export interface DialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The dialog's own heading. It is also what a screen reader announces on open. */
  title: string
  /** Optional supporting line, linked through `aria-describedby`. */
  description?: string
  children?: ReactNode
  /** The actions, usually a confirm and a cancel. */
  footer?: ReactNode
  className?: string
}

/**
 * The overlay, on Radix underneath.
 *
 * Radix is doing three of the four things a hand rolled modal always gets
 * wrong: it traps focus inside while the dialog is open, it closes on Escape,
 * and it hides the rest of the page from assistive technology.
 *
 * The fourth, handing focus back to whatever opened it, is done here, because
 * Radix can only do it for a dialog opened through its own `Trigger`. This one
 * is driven by an `open` prop, so anything at all may have opened it, and
 * without the code below focus lands on `<body>` when the dialog closes. A
 * keyboard user is then dropped at the top of the page, having lost their
 * place. Measured in a test, not assumed: see `forms.test.tsx`.
 *
 * The panel is `w-128`, which compiles to `calc(var(--spacing) * 128)`, so it
 * is 32rem for an admin, 40rem on a phone, and 64rem on a kitchen wall. It
 * renders into a portal on `document.body`, which is exactly why `data-surface`
 * lives on the document element: from out here, a wrapper div's density would
 * be unreachable.
 */
export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  className,
}: DialogProps) {
  const { t } = useTranslation()
  const openerRef = useRef<HTMLElement | null>(null)

  // Tracks what has focus only while the dialog is shut, so by the time it
  // opens the ref already holds whatever was focused just before. Recording it
  // after opening would be too late: focus has moved inside the dialog by then.
  useEffect(() => {
    if (open) return

    const remember = (event: FocusEvent) => {
      if (event.target instanceof HTMLElement) openerRef.current = event.target
    }

    document.addEventListener('focusin', remember)
    return () => {
      document.removeEventListener('focusin', remember)
    }
  }, [open])

  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 bg-overlay" />
        <RadixDialog.Content
          onCloseAutoFocus={(event) => {
            const opener = openerRef.current
            if (!opener?.isConnected) return
            event.preventDefault()
            opener.focus()
          }}
          className={cn(
            'border-line fixed inset-0 m-auto h-fit max-h-[85vh] w-128 max-w-[calc(100%-2rem)]',
            'overflow-auto rounded-lg border-border bg-popover p-6 text-popover-foreground',
            className,
          )}
        >
          <div className="flex items-start gap-4">
            <RadixDialog.Title className="flex-1 text-lg font-semibold">{title}</RadixDialog.Title>
            <Button
              variant="ghost"
              size="icon"
              aria-label={t('dialog.close')}
              onClick={() => {
                onOpenChange(false)
              }}
            >
              <Icon icon={X} size="md" />
            </Button>
          </div>

          {description !== undefined && (
            <RadixDialog.Description className="mt-2 text-sm text-muted-foreground">
              {description}
            </RadixDialog.Description>
          )}

          {children !== undefined && <div className="mt-4">{children}</div>}

          {footer !== undefined && <div className="mt-6 flex flex-wrap gap-3">{footer}</div>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  )
}
