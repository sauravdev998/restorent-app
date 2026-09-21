import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { announce } from '@/shared/ui/announce'
import { isAudioUnlocked } from '@/shared/ui/audio-unlock'

import { playKitchenChime } from './chime'
import { useFreshIds } from './use-fresh-ids'

/** How far down the page counts as "the chef has scrolled away from the top". */
const SCROLLED_AWAY_PX = 120

export interface NewTickets {
  /** The tickets this screen has not shown before, so each card can say so. */
  ids: ReadonlySet<string>
  /** Whether work arrived while the chef was scrolled down, out of sight. */
  aboveTheFold: boolean
  /** Takes the chef back to the top, where the new work is. */
  goToTop: () => void
}

/**
 * Which tickets are new to this screen, and what to do about it.
 *
 * **Seen, then heard.** The card is marked and the announcement goes out before
 * the chime is attempted, and the chime is attempted last and may do nothing at
 * all. A kitchen tablet that nobody has touched since it was plugged in has no
 * audio, so nothing here may depend on the sound (see `chime.ts`).
 *
 * **The scroll is never moved.** A ticket arriving while a chef is reading the
 * bottom of the pass must not slide the screen under their hand mid tap, so this
 * reports that new work is above and offers a way back to it rather than taking
 * them there. New tickets land at the end of the cooking list, which sits above
 * the Ready area, so "above" is where they are whenever the chef has scrolled.
 *
 * The marker is worked out from the two facts rather than remembered, so it can
 * never be left on screen claiming there is new work above after the chef has
 * scrolled up to it or after the next answer has arrived.
 *
 * @param ticketIds every ticket the latest answer carried, or `null` before one
 * has arrived.
 */
export function useNewTickets(ticketIds: readonly string[] | null): NewTickets {
  const { t } = useTranslation('kitchen')
  const ids = useFreshIds(ticketIds)
  const [scrolledAway, setScrolledAway] = useState(false)
  const alerted = useRef<ReadonlySet<string> | null>(null)

  // Announcing and chiming, and nothing else: two external systems, which is what
  // an effect is for. No state is set here, so a ticket arriving costs one render
  // rather than a cascade.
  useEffect(() => {
    if (ids.size === 0 || alerted.current === ids) return
    alerted.current = ids

    // Assertive, because a ticket the kitchen has not started is the one thing on
    // this screen worth interrupting a screen reader for.
    announce(t('pass.newTicket'), 'assertive')

    if (isAudioUnlocked()) playKitchenChime()
  }, [ids, t])

  // Subscribed rather than read during a render, because the scroll position is
  // outside React and changes without it. The state is set from the event, not
  // from the effect's body.
  useEffect(() => {
    const onScroll = (): void => {
      setScrolledAway(window.scrollY > SCROLLED_AWAY_PX)
    }

    window.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      window.removeEventListener('scroll', onScroll)
    }
  }, [])

  return {
    ids,
    aboveTheFold: ids.size > 0 && scrolledAway,
    goToTop: () => {
      window.scrollTo({ top: 0, behavior: 'smooth' })
    },
  }
}
