import { cn } from './cn'

export interface SkeletonProps {
  /**
   * Announced while this stands in for content. Leave it out for the second and
   * later skeletons in a group, so one loading message is announced, not six.
   */
  label?: string
  className?: string
}

/**
 * A shape where content will be.
 *
 * `motion-reduce:animate-none` is the load bearing part: a pulsing rectangle is
 * exactly the kind of repeating movement that makes some people ill, and
 * somebody who has asked their system for less motion has already told us so.
 */
export function Skeleton({ label, className }: SkeletonProps) {
  return (
    <div
      className={cn(
        'h-4 animate-pulse rounded-md bg-muted motion-reduce:animate-none',
        // Its shape is a background, and forced colours removes backgrounds, so
        // it would be a blank gap where content is about to arrive.
        'forced-colors:border-line forced-colors:border-border',
        className,
      )}
      role={label === undefined ? 'presentation' : 'status'}
      aria-hidden={label === undefined ? true : undefined}
    >
      {label !== undefined && <span className="sr-only">{label}</span>}
    </div>
  )
}
