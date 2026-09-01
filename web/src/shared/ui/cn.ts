import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

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
