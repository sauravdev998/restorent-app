import { describe, expect, it } from 'vitest'

import { GROUP_WINDOW_MS, nextAlert, readyDishes, sortOrders, type OrderLike } from './ready'

/**
 * The rules behind the ready alert and the Orders order, as plain functions.
 *
 * Spec 0011, AC-2, AC-9, AC-11: which table comes first, which dishes chime,
 * which join an alert already chiming, and why a reload chimes for nothing.
 */

const ME = 'staff-me'
const COLLEAGUE = 'staff-colleague'

type Status = 'queued' | 'ready' | 'served' | 'voided'

function order(
  id: string,
  options: {
    responsible?: string
    openedAt?: string
    rounds?: { sentAt: string; lines: { id: string; status: Status; readyAt?: string }[] }[]
  } = {},
): OrderLike {
  return {
    id,
    tableLabel: id.toUpperCase(),
    openedAt: options.openedAt ?? '2026-09-18T18:00:00.000Z',
    responsibleStaffId: options.responsible ?? ME,
    rounds: (options.rounds ?? []).map((round) => ({
      sentAt: round.sentAt,
      lines: round.lines.map((line) => ({
        id: line.id,
        dishName: `dish ${line.id}`,
        quantity: 1,
        status: line.status,
        readyAt: line.readyAt ?? null,
      })),
    })),
  }
}

describe('sortOrders', () => {
  it('puts ready food first, longest waiting on top, then cooking, then the rest', () => {
    const idle = order('idle', { openedAt: '2026-09-18T17:00:00.000Z' })
    const idleLater = order('idle-later', { openedAt: '2026-09-18T17:30:00.000Z' })
    const cookingOld = order('cooking-old', {
      rounds: [{ sentAt: '2026-09-18T18:01:00.000Z', lines: [{ id: 'a', status: 'queued' }] }],
    })
    const cookingNew = order('cooking-new', {
      rounds: [{ sentAt: '2026-09-18T18:05:00.000Z', lines: [{ id: 'b', status: 'queued' }] }],
    })
    const readyRecent = order('ready-recent', {
      rounds: [
        {
          sentAt: '2026-09-18T18:00:00.000Z',
          lines: [{ id: 'c', status: 'ready', readyAt: '2026-09-18T18:10:00.000Z' }],
        },
      ],
    })
    const readyLong = order('ready-long', {
      rounds: [
        {
          sentAt: '2026-09-18T18:02:00.000Z',
          lines: [
            { id: 'd', status: 'ready', readyAt: '2026-09-18T18:06:00.000Z' },
            { id: 'e', status: 'queued' },
          ],
        },
      ],
    })

    const sorted = sortOrders([idleLater, cookingNew, readyRecent, idle, cookingOld, readyLong])

    expect(sorted.map((each) => each.id)).toEqual([
      'ready-long',
      'ready-recent',
      'cooking-old',
      'cooking-new',
      'idle',
      'idle-later',
    ])
  }) // covers: AC-2 (spec 0011)

  it('ranks a cooking table by its oldest round that still has a dish cooking', () => {
    // The first round is all served; only the second is still cooking, and it
    // is the one that counts.
    const served = order('mostly-served', {
      rounds: [
        { sentAt: '2026-09-18T18:00:00.000Z', lines: [{ id: 'f', status: 'served' }] },
        { sentAt: '2026-09-18T18:20:00.000Z', lines: [{ id: 'g', status: 'queued' }] },
      ],
    })
    const other = order('other', {
      rounds: [{ sentAt: '2026-09-18T18:10:00.000Z', lines: [{ id: 'h', status: 'queued' }] }],
    })

    expect(sortOrders([served, other]).map((each) => each.id)).toEqual(['other', 'mostly-served'])
  }) // covers: AC-2 (spec 0011)
})

describe('readyDishes', () => {
  it('finds ready dishes on every table, or only on the tables of one waiter', () => {
    const mine = order('mine', {
      rounds: [{ sentAt: '2026-09-18T18:00:00.000Z', lines: [{ id: 'm', status: 'ready' }] }],
    })
    const theirs = order('theirs', {
      responsible: COLLEAGUE,
      rounds: [{ sentAt: '2026-09-18T18:00:00.000Z', lines: [{ id: 't', status: 'ready' }] }],
    })

    expect(readyDishes([mine, theirs]).map((dish) => dish.lineId)).toEqual(['m', 't'])
    expect(readyDishes([mine, theirs], ME).map((dish) => dish.lineId)).toEqual(['m'])
  }) // covers: AC-9 (spec 0011)
})

describe('nextAlert', () => {
  const withReady = (...ids: string[]) =>
    order('t1', {
      rounds: [
        {
          sentAt: '2026-09-18T18:00:00.000Z',
          lines: ids.map((id) => ({ id, status: 'ready' as const })),
        },
      ],
    })

  it('chimes for nothing on the first answer, and remembers what was already ready', () => {
    const first = nextAlert({ seen: null, groupStartedAt: null }, [withReady('a')], ME, 0)

    expect(first.fresh).toEqual([])
    expect(first.chime).toBe(false)
    expect([...first.seen]).toEqual(['a'])
  }) // covers: AC-11 (spec 0011)

  it('chimes for a dish that turns ready on my table, once', () => {
    const start = nextAlert({ seen: null, groupStartedAt: null }, [withReady()], ME, 0)
    const ready = nextAlert(start, [withReady('a')], ME, 1000)
    const again = nextAlert(ready, [withReady('a')], ME, 2000)

    expect(ready.fresh.map((dish) => dish.lineId)).toEqual(['a'])
    expect(ready.chime).toBe(true)
    expect(again.fresh).toEqual([])
    expect(again.chime).toBe(false)
  }) // covers: AC-9 (spec 0011)

  it('joins a dish that turns ready within three seconds to the alert without a second chime', () => {
    const start = nextAlert({ seen: null, groupStartedAt: null }, [withReady()], ME, 0)
    const first = nextAlert(start, [withReady('a')], ME, 10_000)
    const joining = nextAlert(first, [withReady('a', 'b')], ME, 10_000 + GROUP_WINDOW_MS - 1)
    const later = nextAlert(joining, [withReady('a', 'b', 'c')], ME, 10_000 + GROUP_WINDOW_MS + 1)

    expect(joining.fresh.map((dish) => dish.lineId)).toEqual(['b'])
    expect(joining.chime).toBe(false)
    expect(later.fresh.map((dish) => dish.lineId)).toEqual(['c'])
    expect(later.chime).toBe(true)
  }) // covers: AC-9 (spec 0011)

  it('stays silent for a dish on somebody else’s table', () => {
    const theirs = (...ids: string[]) =>
      order('t2', {
        responsible: COLLEAGUE,
        rounds: [
          {
            sentAt: '2026-09-18T18:00:00.000Z',
            lines: ids.map((id) => ({ id, status: 'ready' as const })),
          },
        ],
      })
    const start = nextAlert({ seen: null, groupStartedAt: null }, [theirs()], ME, 0)
    const ready = nextAlert(start, [theirs('x')], ME, 1000)

    expect(ready.fresh).toEqual([])
    expect(ready.chime).toBe(false)
  }) // covers: AC-9 (spec 0011)
})
