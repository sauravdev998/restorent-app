import { useEffect, useRef, useState } from 'react'

/**
 * Which of these ids arrived on the latest answer and no earlier one.
 *
 * **Seeded silently from the first answer.** Everything on the first successful
 * load counts as already known, so a chef who reloads the pass mid service is not
 * told that nine tickets which have been cooking for twenty minutes just arrived.
 *
 * **Emptied again by the next answer that brings nothing new.** The set means
 * "what arrived just now", not "what has ever been new", so a card stops being
 * marked new once the screen has drawn it and asked the server again. Without
 * that, one arrival would leave a New badge on a ticket for the rest of the shift.
 *
 * "New" is a fact about this screen, not about the database, which is why there is
 * no server field for it and should not be: two tablets in one kitchen have
 * different answers, and the one that was switched on a minute ago is not wrong.
 *
 * @param ids every id the latest answer carried, or `null` before one arrived.
 */
export function useFreshIds(ids: readonly string[] | null): ReadonlySet<string> {
  const seen = useRef<ReadonlySet<string> | null>(null)
  const [fresh, setFresh] = useState<ReadonlySet<string>>(() => new Set())

  useEffect(() => {
    if (!ids) return

    const known = seen.current

    // Held on every answer, not only on one that brought something new, so an id
    // whose row has left the screen does not linger and count as new all over
    // again if it comes back.
    seen.current = new Set(ids)

    if (known === null) return

    const arrived = ids.filter((id) => !known.has(id))

    setFresh((current) => {
      // Same answer as last time, so nothing is set and nothing re renders. This
      // is what keeps an answer that brought no new work from queueing a render
      // per refetch on a screen that refetches all evening.
      if (arrived.length === 0 && current.size === 0) return current
      if (arrived.length === current.size && arrived.every((id) => current.has(id))) return current
      return new Set(arrived)
    })
  }, [ids])

  return fresh
}
