import { useSyncExternalStore } from 'react'

/**
 * The "Mine" filter on the waiter's Floor and Orders views (spec 0011, AC-3).
 *
 * On, both views show only the tables whose responsible waiter is the person
 * signed in. Remembered on the phone in local storage under `waiter.mineOnly`,
 * off by default, and read and written inside try and catch so a browser that
 * refuses storage still gets a working switch for the length of the visit.
 *
 * One store shared by both views, so switching it on the floor has it on in
 * Orders too.
 */

/** Where the choice is kept. */
export const MINE_ONLY_STORAGE_KEY = 'waiter.mineOnly'

type Listener = () => void

const listeners = new Set<Listener>()
let mineOnly = read()

function read(): boolean {
  try {
    return window.localStorage.getItem(MINE_ONLY_STORAGE_KEY) === 'true'
  } catch {
    return false
  }
}

/** Switches the filter on or off, and remembers it. */
export function setMineOnly(next: boolean): void {
  mineOnly = next
  try {
    window.localStorage.setItem(MINE_ONLY_STORAGE_KEY, String(next))
  } catch {
    // Remembered for this page only.
  }
  for (const listener of listeners) listener()
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** Whether the filter is on, kept current as it changes. */
export function useMineOnly(): boolean {
  return useSyncExternalStore(subscribe, () => mineOnly)
}

/** Starts again from storage. For tests only. */
export function resetMineOnlyForTests(): void {
  mineOnly = read()
}
