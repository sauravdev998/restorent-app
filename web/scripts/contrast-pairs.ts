/**
 * Every colour pair the design system promises, and what each one has to clear.
 *
 * Pairs of **variable names**, never values. The values live in
 * `src/styles/index.css` and the checker reads them from there, so this file
 * cannot go stale against the palette: rename a role and the check fails,
 * change a colour and the check re measures it.
 *
 * `text` means the pair is text on its background and must clear 4.5:1, the
 * WCAG 2.2 level AA threshold for body text. `interface` means the pair is a
 * boundary, a control's edge, or a focus ring, and must clear 3:1.
 */
export type PairKind = 'text' | 'interface'

export interface ContrastPair {
  /** The variable drawn on top: a text colour, a border, a ring. */
  readonly on: string
  /** The variable behind it. */
  readonly against: string
  readonly kind: PairKind
  /** Why this pair exists, so a failure says what actually breaks. */
  readonly note: string
}

export const THRESHOLDS: Readonly<Record<PairKind, number>> = {
  text: 4.5,
  interface: 3,
}

/** The surfaces anything can end up sitting on. */
const SURFACES = ['background', 'card', 'muted', 'secondary'] as const

function onEverySurface(on: string, note: string): ContrastPair[] {
  return SURFACES.map((against) => ({ on, against, kind: 'text' as const, note }))
}

/** The same, for a mark that is a shape rather than text, which clears 3:1. */
function shapeOnEverySurface(on: string, note: string): ContrastPair[] {
  return SURFACES.map((against) => ({ on, against, kind: 'interface' as const, note }))
}

export const CONTRAST_PAIRS: readonly ContrastPair[] = [
  // Body text, wherever it lands.
  ...onEverySurface('foreground', 'body text'),
  ...onEverySurface('muted-foreground', 'hints, secondary lines, and timestamps'),

  // Every status, on every surface a pill or a row can sit on. A chef reading a
  // ticket from three metres away is the whole reason these are checked here
  // rather than assumed.
  ...onEverySurface('status-queued', 'a dish still cooking'),
  ...onEverySurface('status-ready', 'a dish ready to collect'),
  ...onEverySurface('status-served', 'a dish already taken to the table'),
  ...onEverySurface('status-voided', 'a dish struck off the bill'),
  ...onEverySurface('status-late', 'a round that has waited too long'),

  // The diet marks, on every surface a dish row can sit on. They are shapes
  // rather than text, so they clear the 3:1 a graphic needs; the shape inside
  // the square is what carries the meaning, and the colour only repeats it.
  ...shapeOnEverySurface('diet-veg', 'the veg mark on a dish'),
  ...shapeOnEverySurface('diet-non-veg', 'the non veg mark on a dish'),
  ...shapeOnEverySurface('diet-egg', 'the egg mark on a dish'),

  // Text sitting on its own paired surface.
  { on: 'card-foreground', against: 'card', kind: 'text', note: 'text inside a card' },
  { on: 'popover-foreground', against: 'popover', kind: 'text', note: 'text inside an overlay' },
  {
    on: 'secondary-foreground',
    against: 'secondary',
    kind: 'text',
    note: 'the secondary button label',
  },
  { on: 'accent-foreground', against: 'accent', kind: 'text', note: 'a hovered row or nav item' },
  {
    on: 'primary-foreground',
    against: 'primary',
    kind: 'text',
    note: 'the label on the main action',
  },
  {
    on: 'destructive-foreground',
    against: 'destructive',
    kind: 'text',
    note: 'the label on a destructive action',
  },

  // Action colours used as text, e.g. a link or a ghost button.
  { on: 'primary', against: 'background', kind: 'text', note: 'the action colour as text' },
  { on: 'primary', against: 'card', kind: 'text', note: 'the action colour as text on a card' },
  { on: 'destructive', against: 'background', kind: 'text', note: 'a destructive label as text' },
  { on: 'destructive', against: 'card', kind: 'text', note: 'a destructive label on a card' },

  // Boundaries. These are the ones a design usually gets wrong, because a
  // border that looks tasteful on a designer's monitor is invisible from across
  // a kitchen and gone entirely for someone with low vision.
  { on: 'border', against: 'background', kind: 'interface', note: 'a card or header edge' },
  { on: 'border', against: 'card', kind: 'interface', note: 'a divider inside a card' },
  { on: 'input', against: 'background', kind: 'interface', note: 'a form control edge' },
  { on: 'input', against: 'card', kind: 'interface', note: 'a form control edge inside a card' },
  { on: 'ring', against: 'background', kind: 'interface', note: 'the focus ring' },
  { on: 'ring', against: 'card', kind: 'interface', note: 'the focus ring over a card' },
  { on: 'primary', against: 'background', kind: 'interface', note: 'the main button edge' },
  { on: 'primary', against: 'card', kind: 'interface', note: 'the main button edge on a card' },
  {
    on: 'destructive',
    against: 'background',
    kind: 'interface',
    note: 'the destructive button edge',
  },
  { on: 'destructive', against: 'card', kind: 'interface', note: 'a destructive edge on a card' },
]
