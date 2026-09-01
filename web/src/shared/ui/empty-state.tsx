import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

import { cn } from './cn'
import { Icon } from './icon'

export interface EmptyStateProps {
  icon: LucideIcon
  /** One line saying what is not here. */
  title: string
  /** One line saying why, or what would put something here. */
  description?: string
  /** An optional way out, usually a single `Button`. */
  action?: ReactNode
  className?: string
}

/**
 * The one empty pattern every list in the platform uses.
 *
 * An empty screen is a state, not an absence, and it is the state a new
 * restaurant sees first. Saying what would put something here is the difference
 * between "this is broken" and "nothing has happened yet".
 */
export function EmptyState({ icon, title, description, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        'border-line flex flex-col items-center gap-3 rounded-lg border-dashed border-border p-8 text-center',
        className,
      )}
    >
      <Icon icon={icon} size="lg" className="text-muted-foreground" />
      <p className="text-base font-medium text-foreground">{title}</p>
      {description !== undefined && (
        <p className="max-w-prose text-sm text-muted-foreground">{description}</p>
      )}
      {action}
    </div>
  )
}
