import { useQuery } from '@tanstack/react-query'
import type { TFunction } from 'i18next'
import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { useIdentity } from '@/shared/session/use-identity'
import { announce } from '@/shared/ui/announce'
import { playReadyChime } from '@/shared/ui/audio-unlock'
import { openOrdersQuery } from '@/waiter/api/orders'

import {
  acknowledge,
  acknowledgedSnapshot,
  pruneAcknowledged,
  subscribeToAcknowledged,
} from './acknowledged'
import {
  byTable,
  nextAlert,
  readyDishes,
  REMINDER_MS,
  VIBRATION_PATTERN,
  type ReadyDish,
} from './ready'

/**
 * The waiter's ready alert, for whatever screen of the waiter surface is open
 * (spec 0011, AC-9, AC-10, AC-11).
 *
 * Mounted once, in the waiter shell, so it keeps working while the waiter is
 * on the floor, on Orders, or at another table. It reads the same Orders query
 * the list does, which the live stream refetches whenever any dish moves, so
 * "ready" is always the database's answer and never something this phone
 * remembered.
 *
 * What this phone does remember is small and deliberate:
 *
 * - **which dishes it has already chimed for**, in memory, so each dish alerts
 *   at most once. Seeded on the first answer with every dish already ready,
 *   silently, so a reload never chimes for food that was already up.
 * - **when the current alert started**, so dishes turning ready within three
 *   seconds of it join it without a second chime.
 * - **which dishes the waiter has acknowledged**, in session storage, so the
 *   two minute reminder stops for them.
 *
 * Only the responsible waiter's phone chimes, vibrates, and shows the alert.
 * Every other waiter sees the ready badge on the floor and on Orders, and hears
 * nothing.
 *
 * Returns the dishes on my tables that are ready and not yet acknowledged, and
 * the way to acknowledge them.
 */
export function useReadyAlerts(): {
  pending: ReadyDish[]
  acknowledgeAll: () => void
} {
  const { t } = useTranslation('waiter')
  const me = useIdentity().staff.id
  const orders = useQuery(openOrdersQuery)
  const acknowledged = useSyncExternalStore(subscribeToAcknowledged, acknowledgedSnapshot)

  const memory = useRef<{ seen: ReadonlySet<string> | null; groupStartedAt: number | null }>({
    seen: null,
    groupStartedAt: null,
  })

  useEffect(() => {
    if (!orders.data) return
    const visits = orders.data.visits

    const next = nextAlert(memory.current, visits, me, Date.now())
    memory.current = { seen: next.seen, groupStartedAt: next.groupStartedAt }

    pruneAcknowledged(new Set(readyDishes(visits).map((dish) => dish.lineId)))

    if (next.fresh.length === 0) return

    // Seen first, then said, then heard: the banner is already on screen by
    // the time this runs, the announcement goes out, and only then the chime.
    // A phone on silent, or one whose audio was never unlocked, still gets the
    // first two.
    announce(describe(next.fresh, t), 'assertive')
    if (next.chime) ring()
  }, [orders.data, me, t])

  const mine = useMemo(
    () => (orders.data ? readyDishes(orders.data.visits, me) : []),
    [orders.data, me],
  )
  const pending = useMemo(
    () => mine.filter((dish) => !acknowledged.has(dish.lineId)),
    [mine, acknowledged],
  )

  // One interval for as long as anything is waiting unacknowledged, rather
  // than one per dish, and not restarted every time the list changes: a dish
  // arriving must not push back the reminder for one that has waited a
  // minute and a half already.
  const waiting = pending.length > 0
  useEffect(() => {
    if (!waiting) return

    const timer = window.setInterval(ring, REMINDER_MS)
    return () => {
      window.clearInterval(timer)
    }
  }, [waiting])

  return {
    pending,
    acknowledgeAll: () => {
      acknowledge(pending.map((dish) => dish.lineId))
    },
  }
}

/** The chime and the buzz, both best effort. */
function ring(): void {
  playReadyChime()

  // Feature detected rather than assumed: iOS Safari has no vibration at all,
  // and a call that is not there must not take the chime down with it.
  if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
    try {
      navigator.vibrate(VIBRATION_PATTERN)
    } catch {
      // The alert on screen and the announcement already went out.
    }
  }
}

/** What the live region says: every table, and every dish on it. */
function describe(dishes: readonly ReadyDish[], t: TFunction<'waiter'>): string {
  return byTable(dishes)
    .map((table) =>
      t('alert.announce', {
        label: table.tableLabel,
        dishes: table.dishes
          .map((dish) => t('alert.dish', { quantity: dish.quantity, dish: dish.dishName }))
          .join(', '),
      }),
    )
    .join(' ')
}
