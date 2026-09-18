import { useCallback, useState } from 'react'

import { EMPTY_BASKET, loadBasket, saveBasket, type Basket } from './basket'

/**
 * One visit's unsent basket, kept in session storage as it changes.
 *
 * Read once when the table screen opens for a visit, and written on every
 * change after that, so walking to another screen or reloading brings the
 * basket back exactly as it was (spec 0011, AC-7). Storage that refuses to
 * work leaves the basket in memory, which is all a waiter who does not reload
 * needs.
 *
 * Returns the basket, a way to change it, and a way to clear it for good once
 * a send has landed or the visit has closed.
 */
export function useSavedBasket(visitId: string) {
  const [held, setHeld] = useState<{ visitId: string; basket: Basket }>(() => ({
    visitId,
    basket: loadBasket(visitId),
  }))

  // The same screen reused for a different visit reads that visit's basket,
  // during render rather than in an effect, so the wrong basket is never drawn
  // for even one frame.
  let current = held
  if (held.visitId !== visitId) {
    current = { visitId, basket: loadBasket(visitId) }
    setHeld(current)
  }

  const update = useCallback(
    (change: (basket: Basket) => Basket) => {
      setHeld((previous) => {
        const basket = change(previous.visitId === visitId ? previous.basket : loadBasket(visitId))
        saveBasket(visitId, basket)
        return { visitId, basket }
      })
    },
    [visitId],
  )

  const clear = useCallback(
    (forVisit: string) => {
      saveBasket(forVisit, EMPTY_BASKET)
      if (forVisit === visitId) setHeld({ visitId, basket: EMPTY_BASKET })
    },
    [visitId],
  )

  return { basket: current.basket, update, clear }
}
