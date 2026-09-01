import { Ban, Check, CircleCheck, Flame, TriangleAlert, type LucideIcon } from 'lucide-react'

/**
 * The four states an order line or a round can be in, plus one derived
 * emphasis.
 *
 * `queued`, `ready`, `served`, and `voided` are the real `line_status` and
 * `round_status` values from spec 0003. `late` is not a database state and
 * never will be: it is a presentation emphasis a screen applies to a round that
 * has been waiting too long. Nothing here invents a state the database does not
 * have.
 */
export const ORDER_STATUSES = ['queued', 'ready', 'served', 'voided'] as const

export type OrderStatus = (typeof ORDER_STATUSES)[number]

/** Every status the design system can draw, including the derived emphasis. */
export type StatusTone = OrderStatus | 'late'

export const STATUS_TONES: readonly StatusTone[] = [...ORDER_STATUSES, 'late']

export interface StatusPresentation {
  /** The Tailwind text colour class, which resolves to a `--status-*` token. */
  readonly text: string
  /** The matching border colour class. */
  readonly border: string
  /** The icon that carries the meaning when colour is gone. */
  readonly icon: LucideIcon
  /** The key of the translated word. Never a hard coded string. */
  readonly labelKey: string
  /** A voided line reads as struck through even in a monochrome print out. */
  readonly dashed: boolean
}

/**
 * Colour, word, and icon for every status, in one place.
 *
 * Three signals together, always. Remove the colour and the icon and the word
 * still tell a colour blind chef, a greyscale print out, and a screen in forced
 * colours mode exactly which state a dish is in.
 */
export const STATUS_PRESENTATION: Readonly<Record<StatusTone, StatusPresentation>> = {
  queued: {
    text: 'text-status-queued',
    border: 'border-status-queued',
    icon: Flame,
    labelKey: 'status.queued',
    dashed: false,
  },
  ready: {
    text: 'text-status-ready',
    border: 'border-status-ready',
    icon: CircleCheck,
    labelKey: 'status.ready',
    dashed: false,
  },
  served: {
    text: 'text-status-served',
    border: 'border-status-served',
    icon: Check,
    labelKey: 'status.served',
    dashed: false,
  },
  voided: {
    text: 'text-status-voided',
    border: 'border-status-voided',
    icon: Ban,
    labelKey: 'status.voided',
    dashed: true,
  },
  late: {
    text: 'text-status-late',
    border: 'border-status-late',
    icon: TriangleAlert,
    labelKey: 'status.late',
    dashed: false,
  },
}
