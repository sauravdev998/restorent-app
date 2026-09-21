import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { MemoryRouter, Outlet, Route, Routes } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { kitchenKey } from '@/shared/events/query-keys'
import type { LiveEvents, StreamStatus } from '@/shared/events/use-live-events'
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

type LineStatus = 'queued' | 'ready' | 'served' | 'voided'

interface FixtureLine {
  id: string
  dishName: string
  quantity: number
  note: string | null
  status: LineStatus
  voidReasonCode?: 'guest_changed_mind' | 'entered_by_mistake' | 'kitchen_unavailable' | 'other'
  voidReason?: string | null
}

interface FixtureTicket {
  id: string
  sequenceNo: number
  tableLabel: string
  sentAt: string
  readyAt: string | null
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
    readyAt: null,
    status: 'queued',
    lines: baseLines(),
    ...overrides,
  }
}

/**
 * A whole answer from the pass, with this restaurant's own two thresholds.
 *
 * Ten minutes then fifteen, which are the defaults every restaurant starts with.
 * They are here rather than inside the screen because the screen no longer knows
 * them: they are columns on the restaurant, and the read carries them (spec 0012,
 * AC-3).
 */
function queue(overrides: Record<string, unknown> = {}) {
  return {
    tickets: [ticket()],
    serverTime: SERVER_NOW,
    warningAfterSeconds: 600,
    lateAfterSeconds: 900,
    truncatedCount: 0,
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

/**
 * Finds a dish line by everything it says, even though the dish name inside it
 * sits in its own element marked with the restaurant's language.
 */
function dishLine(text: string) {
  return (_content: string, element: Element | null) =>
    element?.tagName === 'P' && element.textContent === text
}

/** Answers the queue with whatever a test set up. */
function respondWith(answer: unknown) {
  vi.mocked(api.GET).mockResolvedValue({ data: answer })
}

/** Answers the queue with a failure, for the error state. */
function failWith(body: unknown) {
  vi.mocked(api.GET).mockResolvedValue({ error: body })
}

/**
 * The screen inside a query client, a router, and the live stream's context.
 *
 * The router is there because the pass carries the kitchen's tab links, and a
 * link needs to know where it is to say which tab is current. The nested route is
 * there because the pass reads the stream's state from the outlet context, which
 * is where `RootLayout` puts it: one stream per browser, read by whichever screen
 * is open. A bare render would hand the screen no context at all, which is a
 * shape the real app never has.
 */
function wrap(ui: ReactElement, stream: StreamStatus = 'open') {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const live: LiveEvents = { status: stream, last: null, received: 0 }

  return {
    queryClient,
    ui: (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/kitchen']}>
          <Routes>
            <Route path="/kitchen" element={<Outlet context={live} />}>
              <Route index element={ui} />
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  }
}

/**
 * Mounts the screen with the toast viewport beside it.
 *
 * `SurfaceShell` mounts that viewport once per document in the real app, so a
 * screen rendered without it has nowhere for a refusal to land and the message
 * would be missed rather than proved absent.
 */
async function mount(stream: StreamStatus = 'open') {
  const { queryClient, ui } = wrap(
    <>
      <KitchenHome />
      <ToastViewport />
    </>,
    stream,
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
    respondWith(queue())
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    expect(screen.getByText('Round 1')).toBeInTheDocument()
    expect(screen.getByText(dishLine('2 × Tomato soup'))).toBeInTheDocument()
    expect(screen.getByText(dishLine('1 × Butter naan'))).toBeInTheDocument()
  }) // covers: AC-5, AC-6

  it('keeps the queue in the order the server sent it, oldest first', async () => {
    // The server owns the ordering, and this screen must not re-sort. A pass
    // that shuffled its own tickets would put the oldest order anywhere.
    respondWith(
      queue({
        tickets: [
          ticket({ id: 'a', sequenceNo: 1, tableLabel: '1', sentAt: '2026-09-08T12:00:00.000Z' }),
          ticket({ id: 'b', sequenceNo: 2, tableLabel: '2', sentAt: '2026-09-08T12:02:00.000Z' }),
          ticket({ id: 'c', sequenceNo: 3, tableLabel: '3', sentAt: '2026-09-08T12:04:00.000Z' }),
        ],
      }),
    )
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 1')).toBeInTheDocument()
    })

    const labels = screen.getAllByText(/^Table \d$/).map((node) => node.textContent)
    expect(labels).toEqual(['Table 1', 'Table 2', 'Table 3'])
  }) // covers: AC-6

  it('carries a guest’s note to the pass exactly as it was written', async () => {
    respondWith(
      queue({
        tickets: [
          ticket({
            lines: [{ ...at(baseLines(), 0), note: 'no onions, allergy' }],
          }),
        ],
      }),
    )
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

    respondWith(queue())

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
    respondWith(queue())
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
    respondWith(queue())

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
    expect(screen.getByText(dishLine('2 × Tomato soup'))).toBeInTheDocument()

    await act(async () => {
      settle?.()
      await Promise.resolve()
    })
  }) // covers: AC-7

  it('offers no button for a dish already off the pass', async () => {
    respondWith(
      queue({
        tickets: [
          ticket({
            status: 'queued',
            lines: [{ ...at(baseLines(), 0), status: 'ready' }, at(baseLines(), 1)],
          }),
        ],
      }),
    )
    await mount()

    await waitFor(() => {
      expect(screen.getByText(dishLine('2 × Tomato soup'))).toBeInTheDocument()
    })

    // One dish done, one still to cook, and the ticket is still cooking:
    // a chef must not be sent out with half an order.
    expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(1)
    expect(screen.getAllByText('Ready').length).toBeGreaterThan(0)
    // Twice over since spec 0012: the heading of the cooking area, and the
    // ticket's own status pill inside it.
    expect(screen.getAllByText('Cooking').length).toBeGreaterThan(0)
    expect(screen.getByRole('heading', { name: 'Cooking' })).toBeInTheDocument()
  }) // covers: AC-7

  it('tells a chef who lost a race what happened, in words they can read', async () => {
    // `line_not_queued` is what a second chef gets when somebody marked the
    // dish first. The screen must show the sentence for that code, never the
    // API's English, which exists for a log.
    respondWith(queue())
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
    respondWith(queue())
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
    respondWith(queue({ tickets: [] }))
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Nothing on the pass')).toBeInTheDocument()
    })

    expect(
      screen.getByText('New tickets appear here on their own, with no refresh.'),
    ).toBeInTheDocument()
  }) // covers: AC-6

  it('keeps a cancelled dish on its ticket, struck through, with nothing to tap', async () => {
    respondWith(
      queue({
        tickets: [
          ticket({
            lines: [{ ...at(baseLines(), 0), status: 'voided' }, at(baseLines(), 1)],
          }),
        ],
      }),
    )
    await mount()

    await waitFor(() => {
      expect(screen.getByText(dishLine('2 × Tomato soup'))).toBeInTheDocument()
    })

    expect(screen.getByText(dishLine('2 × Tomato soup'))).toHaveClass('line-through')
    expect(screen.getByRole('heading', { name: 'Cancelled' })).toBeInTheDocument()
    // Only the naan, still cooking, can be marked done.
    expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(1)
  }) // covers: AC-14 (spec 0011), AC-12

  it('shows a 140 character note whole and wrapped, never cut off', async () => {
    const note = `No onions, nut allergy. ${'x'.repeat(116)}`
    respondWith(
      queue({
        tickets: [ticket({ lines: [{ ...at(baseLines(), 0), note }, at(baseLines(), 1)] })],
      }),
    )
    await mount()

    const shown = await screen.findByText(note)
    expect(note).toHaveLength(140)
    expect(shown).not.toHaveClass('truncate')
    expect(shown).toHaveClass('whitespace-pre-wrap')
  }) // covers: AC-6, AC-14 (spec 0011)

  it('keeps plated food in its own area until a waiter collects it', async () => {
    // The gap this closes. Before spec 0012 a ticket vanished at the moment it
    // most needed watching, and the only thing tracking uncollected food was a
    // waiter's phone in an apron.
    respondWith(
      queue({
        tickets: [
          ticket({
            id: 'plated',
            status: 'ready',
            readyAt: '2026-09-08T12:04:30.000Z',
            lines: baseLines().map((line) => ({ ...line, status: 'ready' })),
          }),
        ],
      }),
    )
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    expect(screen.getByRole('heading', { name: 'Ready to collect' })).toBeInTheDocument()
    // Its age is measured from when it was plated, not from when it was sent,
    // because what matters about it is how long it has been under the lamp.
    expect(screen.getByText('Plated')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Cooking' })).toBeInTheDocument()
    expect(screen.getByText('Nothing on the pass')).toBeInTheDocument()
  }) // covers: AC-5, AC-6

  it('turns a waiting ticket amber at the restaurant’s own threshold, and red at its own', async () => {
    // Neither number is written into this screen. Both come from the read, which
    // reads them from the restaurant's row, so a kitchen that plates in three
    // minutes and one that roasts for forty are not judged by one clock.
    vi.useFakeTimers()
    vi.setSystemTime(new Date(SERVER_NOW))

    respondWith(
      queue({
        // Twelve minutes old: past the amber threshold of ten, short of the red
        // one at fifteen.
        tickets: [ticket({ sentAt: '2026-09-08T11:53:00.000Z' })],
        warningAfterSeconds: 600,
        lateAfterSeconds: 900,
      }),
    )

    const { queryClient, ui } = wrap(<KitchenHome />)
    render(ui)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0)
    })

    const clock = screen.getByText(/^\d+:\d\d$/).closest('time')
    expect(clock).toHaveAttribute('data-urgency', 'warning')
    expect(clock).not.toHaveAttribute('data-late')

    queryClient.clear()
  }) // covers: AC-3

  it('lets a chef take a tap back, naming the dish it puts back on the stove', async () => {
    respondWith(
      queue({
        tickets: [
          ticket({ lines: [{ ...at(baseLines(), 0), status: 'ready' }, at(baseLines(), 1)] }),
        ],
      }),
    )
    vi.mocked(api.POST).mockResolvedValue({ data: {} })

    const user = userEvent.setup()
    await mount()

    const undo = await screen.findByRole('button', {
      name: 'Put Tomato soup back on the stove',
    })
    await user.click(undo)

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/order-lines/{id}/unready', {
        params: { path: { id: SOUP_LINE } },
      })
    })
  }) // covers: AC-7

  it('clears a whole ticket in one request, never one per dish', async () => {
    // One request and one transaction, so the ticket either fully flips or does
    // not change at all. Four requests is how a chef ends up with two dishes
    // marked and two not because the tablet lost the network half way through.
    respondWith(queue())
    vi.mocked(api.POST).mockResolvedValue({ data: {} })

    const user = userEvent.setup()
    await mount()

    const allDone = await screen.findByRole('button', {
      name: 'Mark every cooking dish on table 7 done',
    })
    await user.click(allDone)

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledTimes(1)
    })

    expect(api.POST).toHaveBeenCalledWith('/api/rounds/{id}/ready', {
      params: { path: { id: '00000000-0000-7000-8000-000000000020' } },
    })
  }) // covers: AC-9

  it('takes a dish off with the one reason a kitchen may give, and no dialog to get wrong', async () => {
    // A chef voids because the kitchen has run out, and for nothing else. The
    // reason is not a choice on this screen, because there is nothing for a chef
    // to choose: why a guest changed their mind is not a judgement to make from
    // behind the pass.
    respondWith(queue())
    vi.mocked(api.POST).mockResolvedValue({ data: {} })

    const user = userEvent.setup()
    await mount()

    const ranOut = await screen.findByRole('button', {
      name: 'Take Tomato soup off, the kitchen has run out',
    })
    await user.click(ranOut)

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/order-lines/{id}/void', {
        params: { path: { id: SOUP_LINE } },
        body: { reasonCode: 'kitchen_unavailable' },
      })
    })
  }) // covers: AC-10, AC-11

  it('says why a dish was cancelled, in the reader’s own language', async () => {
    respondWith(
      queue({
        tickets: [
          ticket({
            lines: [
              {
                ...at(baseLines(), 0),
                status: 'voided',
                voidReasonCode: 'kitchen_unavailable',
                voidReason: null,
              },
              at(baseLines(), 1),
            ],
          }),
        ],
      }),
    )
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Cancelled' })).toBeInTheDocument()
    })

    // From the code, through a translation key. The API's own English is for a
    // log, and a Hindi kitchen must not read it.
    expect(screen.getByText("Cancelled: Kitchen can't make it")).toBeInTheDocument()
  }) // covers: AC-12

  it('says work is hidden rather than hiding it silently', async () => {
    respondWith(queue({ truncatedCount: 7 }))
    await mount()

    await waitFor(() => {
      expect(
        screen.getByText('7 more tickets are not shown. Clear some work to see them.'),
      ).toBeInTheDocument()
    })
  }) // covers: AC-19

  it('says so loudly when it has stopped receiving, and dims the work', async () => {
    // A quiet kitchen screen and a broken kitchen screen look identical, which is
    // the whole reason this state is drawn at all. Nothing is disabled: every act
    // is an ordinary request and works perfectly well with the stream down.
    respondWith(queue())
    await mount('closed')

    await waitFor(() => {
      expect(screen.getByTestId('pass-not-live')).toBeInTheDocument()
    })

    expect(screen.getByText('This screen is not live')).toBeInTheDocument()
    expect(screen.getByTestId('pass-not-live').textContent).toContain('still works')
    expect(screen.getAllByRole('button', { name: 'Done' })).toHaveLength(2)
  }) // covers: AC-17

  it('clears the not live banner once the stream is back', async () => {
    respondWith(queue())
    await mount('open')

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    expect(screen.queryByTestId('pass-not-live')).not.toBeInTheDocument()
  }) // covers: AC-17

  it('offers to turn sound on while the browser is still refusing it', async () => {
    // Audio is locked until somebody touches the page, and a kitchen tablet can
    // sit untouched for an hour. The prompt makes the silence visible and
    // fixable; nothing on this screen depends on the sound.
    respondWith(queue())
    await mount()

    await waitFor(() => {
      expect(screen.getByTestId('pass-sound-off')).toBeInTheDocument()
    })

    expect(screen.getByText('Sound is off')).toBeInTheDocument()
  }) // covers: AC-15

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
    respondWith(queue())

    await expectAccessible(wrap(<KitchenHome />).ui)
  }) // covers: AC-19
})
