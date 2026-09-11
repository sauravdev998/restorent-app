import * as RadixSwitch from '@radix-ui/react-switch'

import { cn } from './cn'

export interface SwitchProps {
  /** Whether it is on. */
  checked: boolean
  /** Called with the value the person asked for. Never called while disabled. */
  onCheckedChange: (checked: boolean) => void
  /**
   * The accessible name: what is being switched, such as "Paneer tikka
   * available". A screen reader reads it, then "switch", then on or off.
   */
  label: string
  /**
   * Greys it and ignores it, while leaving it in the tab order and announced.
   *
   * Deliberately `aria-disabled` rather than the native attribute, the same as
   * `Button`: a natively disabled control cannot be focused, so a keyboard user
   * would never learn it is there or why it will not move.
   */
  disabled?: boolean
  className?: string
}

/**
 * A control that switches one thing on or off, on Radix underneath.
 *
 * Radix gives it the part a hand rolled one gets wrong: a real `button` with
 * `role="switch"` and `aria-checked`, operated by Space and Enter. What this
 * adds is the design system's promises.
 *
 * **The hit area is the surface's, not the track's.** The button itself is
 * `target-min` tall and wide, and the visible track sits inside it, so the
 * same switch is easy to hit with a mouse at a desk and with a wet thumb on a
 * kitchen wall. The track is sized in spacing steps, so it grows with the
 * density like everything else.
 *
 * **On and off are never colour alone.** The thumb moves, and it moves the
 * other way under `dir="rtl"`. A caller shows the state in words beside it as
 * well, which is what a chef reads from across the room.
 *
 * **It survives forced colours.** The track is a real border. The thumb opts
 * out of colour forcing and paints itself in the system's own text colour, and
 * in `Highlight` when on, because forced colours removes backgrounds and would
 * otherwise leave an empty track with no thumb in it.
 */
export function Switch({
  checked,
  onCheckedChange,
  label,
  disabled = false,
  className,
}: SwitchProps) {
  return (
    <RadixSwitch.Root
      checked={checked}
      onCheckedChange={(next) => {
        if (!disabled) onCheckedChange(next)
      }}
      aria-label={label}
      aria-disabled={disabled || undefined}
      className={cn(
        'group target-min inline-flex shrink-0 items-center justify-center rounded-md',
        'aria-disabled:cursor-not-allowed aria-disabled:opacity-50',
        'forced-colors:aria-disabled:opacity-100',
        className,
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          'border-line relative inline-flex h-6 w-11 items-center rounded-full border-input bg-muted',
          'transition-colors',
          'group-data-[state=checked]:border-primary group-data-[state=checked]:bg-primary',
          'forced-colors:group-data-[state=checked]:border-[Highlight]',
          'forced-colors:group-aria-disabled:border-[GrayText]',
        )}
      >
        <RadixSwitch.Thumb
          className={cn(
            'block size-4 rounded-full bg-foreground transition-transform',
            'translate-x-1 data-[state=checked]:translate-x-5.5',
            'rtl:-translate-x-1 rtl:data-[state=checked]:-translate-x-5.5',
            'data-[state=checked]:bg-primary-foreground',
            'forced-colors:forced-color-adjust-none forced-colors:bg-[CanvasText]',
            'forced-colors:data-[state=checked]:bg-[Highlight]',
            'forced-colors:group-aria-disabled:bg-[GrayText]',
          )}
        />
      </span>
    </RadixSwitch.Root>
  )
}
