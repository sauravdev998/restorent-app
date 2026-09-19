/**
 * Which ready dishes this phone has acknowledged, so their reminders stop.
 *
 * A set of line ids, added to by the alert's Acknowledge button and by opening
 * a table's screen, and pruned of any id that is no longer ready (spec 0011,
 * AC-10). Kept in session storage under `waiter.acknowledged`, so a reload
 * does not start the reminders again for food the waiter already knows about.
 *
 * If storage will not work the set lives in memory, so acknowledging still
 * stops the reminders until a reload.
 *
 * A small store rather than React state, because two unrelated parts of the
 * waiter surface write to it (the alert in the shell and the table screen) and
 * both have to see the same answer at once.
 */

/** Where the set is kept. */
export const ACKNOWLEDGED_STORAGE_KEY = 'waiter.acknowledged'

type Listener = () => void

const listeners = new Set<Listener>()
let acknowledged: ReadonlySet<string> = read()

function read(): ReadonlySet<string> {
  try {
    const raw = window.sessionStorage.getItem(ACKNOWLEDGED_STORAGE_KEY)
    if (raw === null) return new Set()
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return new Set()
    return new Set((parsed as unknown[]).filter((id): id is string => typeof id === 'string'))
  } catch {
    return new Set()
  }
}

function write(next: ReadonlySet<string>): void {
  if (next.size === acknowledged.size && [...next].every((id) => acknowledged.has(id))) return

  acknowledged = next
  try {
    window.sessionStorage.setItem(ACKNOWLEDGED_STORAGE_KEY, JSON.stringify([...next]))
  } catch {
    // Kept in memory only. Reminders still stop until a reload.
  }
  for (const listener of listeners) listener()
}

/** The current set. The same object until something changes it. */
export function acknowledgedSnapshot(): ReadonlySet<string> {
  return acknowledged
}

/** Marks these dishes as seen by the waiter holding this phone. */
export function acknowledge(lineIds: Iterable<string>): void {
  const next = new Set(acknowledged)
  for (const id of lineIds) next.add(id)
  write(next)
}

/**
 * Forgets every id that is no longer a ready dish, so the set does not grow
 * all evening, and so a dish that somehow turns ready again is alerted again.
 */
export function pruneAcknowledged(readyLineIds: ReadonlySet<string>): void {
  write(new Set([...acknowledged].filter((id) => readyLineIds.has(id))))
}

/** Subscribes to changes. Returns a function that unsubscribes. */
export function subscribeToAcknowledged(listener: Listener): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** Starts again from storage. For tests only. */
export function resetAcknowledgedForTests(): void {
  acknowledged = read()
}
