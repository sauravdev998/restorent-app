import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { kitchenKey } from '@/shared/events/query-keys'
import { ToastViewport } from '@/shared/ui/toast'
import { expectAccessible } from '@/test/axe'

import { KitchenHome } from './kitchen-home'

/**
 * The pass, and the three things about it that are easy to get wrong.
 *
 * **The age is the server's, not the tablet's.** A kitchen screen is a cheap
 * appliance whose clock nobody sets and nobody looks at. The one number a chef
 * acts on is how long a ticket has waited, so it is measured against the time
 * the response itself reports. A screen reading its own clock would tell a
 * kitchen a one minute old ticket had been sitting there for twenty.
 *
 * **A tap marks one dish and writes nothing early.** The ticket's status is the
 * server's to compute from its dishes. A screen that greyed a dish out on the
 * tap would lie for as long as the request took, and keep lying if it failed.
 *
 * **A refusal is read in the reader's own language.** The API answers with a
 * code and an English sentence meant for a log. Rendering that sentence would
 * put English in the middle of a Hindi screen and make the API responsible for
 * the product's wording.
 *
 * The client is replaced wholesale, the way the other screen tests do it: the
 * real one builds a `Request` from a relative address, which jsdom refuses.
 * What is under test is what this screen does with an answer.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const SOUP_LINE = '00000000-0000-7000-8000-000000000031'
const NAAN_LINE = '00000000-0000-7000-8000-000000000032'

/** The moment the server says it is, for every fixture in this file. */
const SERVER_NOW = '2026-09-08T12:05:00.000Z'

type LineStatus = 'queued' | 'ready' | 'served'

interface FixtureLine {
  id: string
  dishName: string
  quantity: number
  note: string | null
  status: LineStatus
}

interface FixtureTicket {
  id: string
  sequenceNo: number
  tableLabel: string
  sentAt: string
  status: LineStatus
  lines: FixtureLine[]
}

/** The two dishes every ticket in this file starts with, both still to cook. */
function baseLines(): FixtureLine[] {
  return [
    { id: SOUP_LINE, dishName: 'Tomato soup', quantity: 2, note: null, status: 'queued' },
    { id: NAAN_LINE, dishName: 'Butter naan', quantity: 1, note: null, status: 'queued' },
  ]
}

/** A ticket sent one minute before the server's own clock reading. */
function ticket(overrides: Partial<FixtureTicket> = {}): FixtureTicket {
  return {
    id: '00000000-0000-7000-8000-000000000020',
    sequenceNo: 1,
    tableLabel: '7',
    sentAt: '2026-09-08T12:04:00.000Z',
    status: 'queued',
    lines: baseLines(),
    ...overrides,
  }
}

/**
 * The nth element, or a failed test.
 *
 * `noUncheckedIndexedAccess` is on and non null assertions are a lint error, so
 * this is how a test reaches into a list. It also turns an off by one into a
 * sentence rather than a confusing `undefined`.
 */
function at<T>(items: readonly T[], index: number): T {
  const item = items[index]
  if (item === undefined) throw new Error(`no element at index ${index}`)
  return item
}

/** Answers the queue with whatever a test set up. */
function respondWith(queue: unknown) {
  vi.mocked(api.GET).mockResolvedValue({ data: queue })
}

/** Answers the queue with a failure, for the error state. */
function failWith(body: unknown) {
  vi.mocked(api.GET).mockResolvedValue({ error: body })
}

function wrap(ui: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return { queryClient, ui: <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider> }
}

/**
 * Mounts the screen with the toast viewport beside it.
 *
 * `SurfaceShell` mounts that viewport once per document in the real app, so a
 * screen rendered without it has nowhere for a refusal to land and the message
 * would be missed rather than proved absent.
 */
async function mount() {
  const { queryClient, ui } = wrap(
    <>
      <KitchenHome />
      <ToastViewport />
    </>,
  )
  const view = render(ui)

  await act(async () => {
    await Promise.resolve()
  })

  return { ...view, queryClient }
}

beforeEach(() => {
  vi.clearAllMocks()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('KitchenHome', () => {
  it('shows a ticket with its table, its round, and each dish at its quantity', async () => {
    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    expect(screen.getByText('Round 1')).toBeInTheDocument()
    expect(screen.getByText('2 × Tomato soup')).toBeInTheDocument()
    expect(screen.getByText('1 × Butter naan')).toBeInTheDocument()
  }) // covers: AC-5, AC-6

  it('keeps the queue in the order the server sent it, oldest first', async () => {
    // The server owns the ordering, and this screen must not re-sort. A pass
    // that shuffled its own tickets would put the oldest order anywhere.
    respondWith({
      tickets: [
        ticket({ id: 'a', sequenceNo: 1, tableLabel: '1', sentAt: '2026-09-08T12:00:00.000Z' }),
        ticket({ id: 'b', sequenceNo: 2, tableLabel: '2', sentAt: '2026-09-08T12:02:00.000Z' }),
        ticket({ id: 'c', sequenceNo: 3, tableLabel: '3', sentAt: '2026-09-08T12:04:00.000Z' }),
      ],
      serverTime: SERVER_NOW,
    })
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 1')).toBeInTheDocument()
    })

    const labels = screen.getAllByText(/^Table \d$/).map((node) => node.textContent)
    expect(labels).toEqual(['Table 1', 'Table 2', 'Table 3'])
  }) // covers: AC-6

  it('carries a guest’s note to the pass exactly as it was written', async () => {
    respondWith({
      tickets: [
        ticket({
          lines: [{ ...at(baseLines(), 0), note: 'no onions, allergy' }],
        }),
      ],
      serverTime: SERVER_NOW,
    })
    await mount()

    await waitFor(() => {
      expect(screen.getByText('no onions, allergy')).toBeInTheDocument()
    })
  }) // covers: AC-6

  it('measures the age against the server’s clock, not the tablet’s', async () => {
    // The failure this pins. The device is twenty minutes fast; the ticket was
    // sent one minute before the server's own reading. A screen reading its own
    // clock would say twenty one minutes and send a chef chasing a ticket that
    // is barely a minute old.
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-09-08T12:25:00.000Z'))

    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })

    const { queryClient, ui } = wrap(<KitchenHome />)
    render(ui)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })

    expect(screen.getByText('Table 7')).toBeInTheDocument()

    // One minute, give or take the tick, and nowhere near twenty one.
    const elapsed = screen.getByText(/^\d+:\d\d$/).textContent ?? ''
    const [minutes] = elapsed.split(':')
    expect(
      Number(minutes),
      `the pass read ${elapsed}, which is the device's clock rather than the server's`,
    ).toBeLessThan(5)

    queryClient.clear()
  }) // covers: AC-6

  it('marks one dish and leaves every other dish alone', async () => {
    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })
    vi.mocked(api.POST).mockResolvedValue({ data: {} })

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(2)
    })

    await user.click(at(screen.getAllByRole('button', { name: 'Done' }), 0))

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledTimes(1)
    })

    // The tapped dish, by id, and nothing that names the ticket or the round:
    // the ticket's own status follows from its dishes on the server.
    expect(api.POST).toHaveBeenCalledWith('/api/order-lines/{id}/ready', {
      params: { path: { id: SOUP_LINE } },
    })
  }) // covers: AC-7

  it('shows the tap as pending without writing the dish off early', async () => {
    // No cache optimism anywhere near this. A dish is done when the pass says
    // it is; a screen that greyed it out on the tap would lie for the length of
    // the request and keep lying if it failed.
    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })

    let settle: (() => void) | undefined
    vi.mocked(api.POST).mockReturnValue(
      new Promise((resolve) => {
        settle = () => {
          resolve({ data: {} })
        }
      }),
    )

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(2)
    })

    await user.click(at(screen.getAllByRole('button', { name: 'Done' }), 0))

    // Its own button says so, and it is the only one that changed. The button
    // carries `aria-disabled` rather than the native attribute on purpose, so
    // the control stays reachable and announced while it waits.
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Marking' })).toHaveAttribute(
        'aria-disabled',
        'true',
      )
    })
    expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(1)

    // And the dish still reads as waiting, because nothing has confirmed it.
    expect(screen.getByText('2 × Tomato soup')).toBeInTheDocument()

    await act(async () => {
      settle?.()
      await Promise.resolve()
    })
  }) // covers: AC-7

  it('offers no button for a dish already off the pass', async () => {
    respondWith({
      tickets: [
        ticket({
          status: 'queued',
          lines: [{ ...at(baseLines(), 0), status: 'ready' }, at(baseLines(), 1)],
        }),
      ],
      serverTime: SERVER_NOW,
    })
    await mount()

    await waitFor(() => {
      expect(screen.getByText('2 × Tomato soup')).toBeInTheDocument()
    })

    // One dish done, one still to cook, and the ticket is still cooking:
    // a chef must not be sent out with half an order.
    expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(1)
    expect(screen.getAllByText('Ready').length).toBeGreaterThan(0)
    expect(screen.getByText('Cooking')).toBeInTheDocument()
  }) // covers: AC-7

  it('tells a chef who lost a race what happened, in words they can read', async () => {
    // `line_not_queued` is what a second chef gets when somebody marked the
    // dish first. The screen must show the sentence for that code, never the
    // API's English, which exists for a log.
    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'line_not_queued', message: 'that dish is no longer waiting to be cooked' },
    })

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(2)
    })

    await user.click(at(screen.getAllByRole('button', { name: 'Done' }), 0))

    await waitFor(() => {
      expect(
        screen.getByText(
          'Another chef marked that dish first. The ticket now shows where it stands.',
        ),
      ).toBeInTheDocument()
    })

    expect(
      screen.queryByText('that dish is no longer waiting to be cooked'),
      'the API’s English reached a chef’s screen',
    ).not.toBeInTheDocument()
  }) // covers: AC-13

  it('refetches the queue after a refusal, so the screen shows where things stand', async () => {
    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'line_not_queued', message: 'no longer waiting' },
    })

    const user = userEvent.setup()
    const { queryClient } = await mount()
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')

    await waitFor(() => {
      expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(2)
    })

    await user.click(at(screen.getAllByRole('button', { name: 'Done' }), 0))

    await waitFor(() => {
      expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: kitchenKey })
    })
  }) // covers: AC-13

  it('says the pass is quiet rather than showing an empty screen', async () => {
    // A blank kitchen screen and a broken kitchen screen look identical from
    // across a room, which is the whole reason this state has words.
    respondWith({ tickets: [], serverTime: SERVER_NOW })
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Nothing on the pass')).toBeInTheDocument()
    })

    expect(
      screen.getByText('New tickets appear here on their own, with no refresh.'),
    ).toBeInTheDocument()
  }) // covers: AC-6

  it('offers a way back when the queue cannot be read', async () => {
    failWith({ error: 'unavailable', message: 'the database is unavailable' })
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument()
    })

    // The code's sentence, not the API's English.
    expect(screen.queryByText('the database is unavailable')).not.toBeInTheDocument()
  }) // covers: AC-13

  it('is accessible in both appearances and at every density', async () => {
    // The gate this screen never had. It ships into the one room where being
    // unreadable hurts most, and it is rendered here at admin, waiter, and
    // kitchen density because the same component is used at all three.
    respondWith({ tickets: [ticket()], serverTime: SERVER_NOW })

    await expectAccessible(
      <QueryClientProvider
        client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
      >
        <KitchenHome />
      </QueryClientProvider>,
    )
  }) // covers: AC-19
})
