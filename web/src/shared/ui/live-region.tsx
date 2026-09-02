import { useEffect, useState } from 'react'

import { subscribeToAnnouncements } from './announce'

/**
 * The two regions every announcement lands in, mounted once by `SurfaceShell`.
 *
 * Both are always in the document and both are empty until something is said,
 * which is the arrangement screen readers actually watch. One polite, for
 * confirmations; one assertive, for the alert that cannot wait.
 */
export function LiveRegion() {
  const [polite, setPolite] = useState('')
  const [assertive, setAssertive] = useState('')

  useEffect(
    () =>
      subscribeToAnnouncements((message, urgency) => {
        const set = urgency === 'assertive' ? setAssertive : setPolite
        // Cleared first, because setting a region to the text it already holds
        // is not a change, and a screen reader announces changes. Two identical
        // "food is ready" alerts in a row must both be heard.
        set('')
        requestAnimationFrame(() => {
          set(message)
        })
      }),
    [],
  )

  return (
    <>
      <div role="status" aria-live="polite" aria-atomic="true" className="sr-only">
        {polite}
      </div>
      <div role="alert" aria-live="assertive" aria-atomic="true" className="sr-only">
        {assertive}
      </div>
    </>
  )
}
