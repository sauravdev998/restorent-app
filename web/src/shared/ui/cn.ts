import { clsx, type ClassValue } from 'clsx'
import { extendTailwindMerge } from 'tailwind-merge'

/**
 * tailwind-merge, taught about the utilities this design system adds.
 *
 * Without this it does real damage, silently. It groups classes by the CSS
 * property they set and keeps only the last one in each group, and it works
 * that out by parsing the class name. `border-line` looks to it like a border
 * *colour*, so `cn('border-line', 'border-border')` returned only
 * `border-border`: the width was thrown away and every card, button, pill, and
 * input lost its boundary. No error, no warning, and it still looked fine at a
 * desk, because a 1px hairline missing is a subtle thing until you are three
 * metres from a kitchen screen or running in forced colours mode, which is
 * exactly where this system promised the border would be.
 *
 * Found by looking at the page in a browser, not by any test. `cn.test.ts`
 * holds the regression.
 */
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      // Border widths that follow the surface, not colours.
      'border-w': ['border-line'],
      'border-w-t': ['border-t-line'],
      'border-w-b': ['border-b-line'],
      // An icon's stroke weight, which is a width and not a colour either.
      'stroke-w': ['stroke-token'],
    },
  },
})

/**
 * Joins class names, letting a later Tailwind utility beat an earlier one.
 *
 * Without the merge, `cn('px-4', 'px-6')` would emit both and leave the winner
 * to whichever rule Tailwind happened to write first. With it, the caller's
 * class wins, which is what makes a `className` prop on a base component
 * actually useful.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
}
