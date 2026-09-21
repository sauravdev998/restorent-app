import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { menuKey } from '@/shared/events/query-keys'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { expectAccessible } from '@/test/axe'
import { acknowledgedSnapshot, resetAcknowledgedForTests } from '@/waiter/alerts/acknowledged'

import { WaiterTable } from './table'

/**
 * The waiter's table screen.
 *
 * The ready alert is no longer this screen's: it lives in the waiter shell
 * and is tested in `alerts/`. What this screen owes is the rest of spec 0011:
 * standing at the table acknowledges its ready food, a dish is served or
 * cancelled on its own, the basket survives a remount and carries its send
 * key, and an empty close reads as nothing to charge.
 *
 * The client is replaced wholesale, the way the other screen tests do it: the
 * real one builds a `Request` from a relative address, which jsdom refuses.
 * What is under test is what this screen does with an answer.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const IDENTITY: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Wes Waiter',
    email: 'wes@example.test',
    role: 'waiter',
    language: null,
    mustChangePassword: false,
  },
  restaurant: {
    id: '00000000-0000-7000-8000-000000000002',
    name: 'The Test Kitchen',
    address: null,
    countryCode: 'IN',
    currencyCode: 'INR',
    currencyDecimals: 2,
    timezone: 'Asia/Kolkata',
    defaultLanguage: 'en',
    formattingLocale: 'en-IN',
    kitchenWarningAfterSeconds: 600,
    kitchenLateAfterSeconds: 900,
    version: 1,
  },
}

const VISIT_ID = '00000000-0000-7000-8000-000000000010'
const ROUND_ID = '00000000-0000-7000-8000-000000000020'
const LINE_ID = '00000000-0000-7000-8000-000000000030'
const COLLEAGUE = '00000000-0000-7000-8000-000000000003'

/** A visit document, with its one round in whatever state a test needs. */
function visitWith(
  roundStatus: 'queued' | 'ready' | 'served',
  options: { responsible?: 'me' | 'colleague'; billStatus?: 'open' | 'voided' } = {},
) {
  const colleague = options.responsible === 'colleague'
  return {
    id: VISIT_ID,
    tableId: '00000000-0000-7000-8000-000000000011',
    tableLabel: '7',
    status: 'open',
    guestCount: 2,
    openedBy: 'Wes Waiter',
    responsibleStaffId: colleague ? COLLEAGUE : IDENTITY.staff.id,
    responsibleName: colleague ? 'Cara Colleague' : 'Wes Waiter',
    openedAt: '2026-09-08T12:00:00.000Z',
    serverTime: '2026-09-08T12:05:00.000Z',
    rounds: [
      {
        id: ROUND_ID,
        visitId: VISIT_ID,
        sequenceNo: 1,
        status: roundStatus,
        sentAt: '2026-09-08T12:01:00.000Z',
        readyAt: roundStatus === 'queued' ? null : '2026-09-08T12:04:00.000Z',
        servedAt: null,
        lines: [
          {
            id: LINE_ID,
            dishId: '00000000-0000-7000-8000-000000000040',
            dishName: 'Tomato soup',
            quantity: 1,
            unitPrice: '180.0000',
            lineTotal: '180.0000',
            note: 'no onions',
            status: roundStatus === 'served' ? 'served' : roundStatus,
            readyAt: roundStatus === 'queued' ? null : '2026-09-08T12:04:00.000Z',
            voidReasonCode: null,
            voidReason: null,
          },
        ],
      },
    ],
    bill: {
      id: '00000000-0000-7000-8000-000000000050',
      number: null,
      status: options.billStatus ?? 'open',
      currencyCode: 'INR',
      currencyDecimals: 2,
      subtotal: '180.0000',
      serviceChargePercent: null,
      serviceChargeAmount: '0',
      taxTotal: '0',
      total: '0',
      closedAt: null,
      taxes: [],
    },
  }
}

/** An empty menu, so the ordering half renders without being the subject. */
const MENU = { categories: [], currencyCode: 'INR', currencyDecimals: 2 }

const TIKKA = '00000000-0000-7000-8000-000000000041'
const SOUP = '00000000-0000-7000-8000-000000000042'

/** A menu of two dishes, the soup on or off, and the tikka there or removed. */
function menuWith({ soup = true, tikka = true }: { soup?: boolean; tikka?: boolean } = {}) {
  return {
    currencyCode: 'INR',
    currencyDecimals: 2,
    categories: [
      {
        id: '00000000-0000-7000-8000-000000000060',
        name: 'Starters',
        dishes: [
          ...(tikka
            ? [
                {
                  id: TIKKA,
                  name: 'Paneer tikka',
                  description: null,
                  price: '320.0000',
                  diet: 'veg',
                  available: true,
                },
              ]
            : []),
          {
            id: SOUP,
            name: 'Tomato soup',
            description: null,
            price: '180.0000',
            diet: 'veg',
            available: soup,
          },
        ],
      },
    ],
  }
}

/** What `GET /api/menu` answers, changeable mid test the way a live event would. */
let currentMenu: unknown = MENU

/** How many times the screen has read the menu. */
let menuReads = 0

/** Answers `GET` from whatever the test set up for each path. */
function respondWith(visit: unknown, menu: unknown = MENU) {
  currentMenu = menu
  vi.mocked(api.GET).mockImplementation((path: string) => {
    if (path === '/api/menu') {
      menuReads += 1
      return Promise.resolve({ data: currentMenu }) as never
    }
    if (path === '/api/me') return Promise.resolve({ data: IDENTITY }) as never
    if (path === '/api/floor') return Promise.resolve({ data: { sections: [] } }) as never
    return Promise.resolve({ data: visit }) as never
  })
}

async function mount() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

  const router = createMemoryRouter(
    [
      {
        id: 'root',
        path: '/waiter/tables/:visitId',
        element: <WaiterTable />,
        loader: () => IDENTITY,
      },
    ],
    { initialEntries: [`/waiter/tables/${VISIT_ID}`] },
  )

  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  await act(async () => {
    await Promise.resolve()
  })

  return { ...view, queryClient }
}

beforeEach(() => {
  vi.clearAllMocks()
  window.sessionStorage.clear()
  resetAcknowledgedForTests()
})

describe('WaiterTable', () => {
  it('says nothing about food being ready while it is still cooking', async () => {
    respondWith(visitWith('queued'))
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    expect(screen.queryByRole('button', { name: /^serve/i })).not.toBeInTheDocument()
  }) // covers: AC-8, AC-10

  it('acknowledges ready food just by being open on its table', async () => {
    // Standing at the table is knowing its food is up, so the reminder stops.
    respondWith(visitWith('ready'))
    await mount()

    await screen.findByText('Table 7')
    await waitFor(() => {
      expect(acknowledgedSnapshot().has(LINE_ID)).toBe(true)
    })
    expect(window.sessionStorage.getItem('waiter.acknowledged')).toContain(LINE_ID)
  }) // covers: AC-10 (spec 0011)

  it('serves one ready dish on its own', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('ready'))
    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Serve Tomato soup' }))

    expect(api.POST).toHaveBeenCalledWith('/api/order-lines/{id}/served', {
      params: { path: { id: LINE_ID } },
    })
  }) // covers: AC-12 (spec 0011)

  it('offers serve all ready on a round with a ready dish', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('ready'))
    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Serve all ready' }))

    expect(api.POST).toHaveBeenCalledWith('/api/rounds/{id}/served', {
      params: { path: { id: ROUND_ID } },
    })
  }) // covers: AC-12 (spec 0011)

  it('shows the note on a dish that was sent with one', async () => {
    respondWith(visitWith('queued'))
    await mount()

    expect(await screen.findByText('no onions')).toBeInTheDocument()
  }) // covers: AC-6 (spec 0011)

  it('cancels a dish only with a reason, and asks for words when the reason is other', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('queued'))
    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Cancel Tomato soup' }))
    const dialog = await screen.findByRole('dialog', { name: 'Cancel this dish?' })

    await user.selectOptions(within(dialog).getByRole('combobox', { name: 'Why' }), 'other')
    await user.click(within(dialog).getByRole('button', { name: 'Cancel the dish' }))

    expect(
      await within(dialog).findByText('Say what happened, so a manager reading this later knows.'),
    ).toBeInTheDocument()
    expect(api.POST).not.toHaveBeenCalled()

    await user.type(within(dialog).getByRole('textbox', { name: /Details/ }), 'spilled')
    await user.click(within(dialog).getByRole('button', { name: 'Cancel the dish' }))

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/order-lines/{id}/void', {
        params: { path: { id: LINE_ID } },
        body: { reasonCode: 'other', reason: 'spilled' },
      })
    })
  }) // covers: AC-13 (spec 0011)

  it('names the responsible waiter and offers to take the table over', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('queued', { responsible: 'colleague' }))
    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    expect(await screen.findByText("Cara Colleague's table")).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Take over' }))

    expect(api.POST).toHaveBeenCalledWith('/api/visits/{id}/take-over', {
      params: { path: { id: VISIT_ID } },
      body: { expectedStaffId: COLLEAGUE },
    })
  }) // covers: AC-4 (spec 0011)

  it('offers no take over on my own table', async () => {
    respondWith(visitWith('queued'))
    await mount()

    expect(await screen.findByText('Your table')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Take over' })).not.toBeInTheDocument()
  }) // covers: AC-4 (spec 0011)

  it('says there is nothing to charge when the empty bill was voided', async () => {
    respondWith({ ...visitWith('served', { billStatus: 'voided' }), status: 'closed' })
    await mount()

    expect(await screen.findByRole('heading', { name: 'Nothing to charge' })).toBeInTheDocument()
    expect(screen.queryByText('Total')).not.toBeInTheDocument()
  }) // covers: AC-16 (spec 0011)

  it('writes money in the bill’s own currency, from the exact decimal', async () => {
    respondWith(visitWith('ready'))
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    // ₹, grouped the Indian way, and two decimals because that is what the
    // bill says its currency uses. Never parsed into a float on the way.
    expect(screen.getAllByText('₹180.00').length).toBeGreaterThan(0)
  }) // covers: AC-10

  it('is accessible, headings and labelled controls included', async () => {
    respondWith(visitWith('ready'))

    await expectAccessible(
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider
          router={createMemoryRouter(
            [
              {
                id: 'root',
                path: '/waiter/tables/:visitId',
                element: <WaiterTable />,
                loader: () => IDENTITY,
              },
            ],
            { initialEntries: [`/waiter/tables/${VISIT_ID}`] },
          )}
        />
      </QueryClientProvider>,
    )
  }) // covers: AC-19
})

/**
 * The basket against a menu that moves under it, spec 0008 AC-12.
 *
 * The live event is stood in for by changing what the menu read answers and
 * invalidating the menu, which is exactly what the stream does on a `dish`
 * event.
 */
describe('WaiterTable basket', () => {
  it('flags a line the kitchen switches off, and blocks sending until it is out', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    const { queryClient } = await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Tomato soup' }))
    await user.click(screen.getByRole('button', { name: 'Add one Paneer tikka' }))
    expect(screen.getByRole('button', { name: /^send 2 dishes$/i })).not.toHaveAttribute(
      'aria-disabled',
    )

    // The chef switches the soup off; the stream invalidates the menu.
    currentMenu = menuWith({ soup: false })
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: menuKey })
    })

    expect(
      await screen.findByText(
        '1 dish in the basket has just gone off. Take it out to send the rest.',
      ),
    ).toHaveAttribute('role', 'status')
    expect(screen.getByText('Off now, cannot be sent')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^send 2 dishes$/i })).toHaveAttribute(
      'aria-disabled',
      'true',
    )

    await user.click(screen.getByRole('button', { name: 'Take Tomato soup out of the basket' }))

    expect(screen.queryByText('Off now, cannot be sent')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^send 1 dish$/i })).not.toHaveAttribute(
      'aria-disabled',
    )
  }) // covers: AC-12 (spec 0008)

  it('flags a line whose dish was taken off the menu, by the name it went in with', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    const { queryClient } = await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Paneer tikka' }))

    currentMenu = menuWith({ tikka: false })
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: menuKey })
    })

    // Gone from the menu, so the basket is the only place its name survives.
    expect(
      await screen.findByRole('button', { name: 'Take Paneer tikka out of the basket' }),
    ).toBeInTheDocument()
  }) // covers: AC-12 (spec 0008)

  it('reads the menu again when a send is refused because a dish went off', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'dish_not_orderable', message: 'a dish in the basket cannot be ordered' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Tomato soup' }))

    // The send races past the flag: the server's menu already has it off.
    currentMenu = menuWith({ soup: false })
    const before = menuReads

    await user.click(screen.getByRole('button', { name: /^send 1 dish$/i }))

    await waitFor(() => {
      expect(menuReads).toBeGreaterThan(before)
    })
    expect(await screen.findByText('Off now, cannot be sent')).toBeInTheDocument()
  }) // covers: AC-12 (spec 0008)
})

/** The basket that survives, spec 0011 AC-6, AC-7, AC-8. */
describe('WaiterTable basket on the phone', () => {
  it('survives leaving the screen, and sends its note and its key', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    const first = await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Tomato soup' }))
    await user.type(screen.getByRole('textbox', { name: 'Note for Tomato soup' }), 'no onions')
    first.unmount()

    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    const note = await screen.findByRole('textbox', { name: 'Note for Tomato soup' })
    expect(note).toHaveValue('no onions')

    await user.click(screen.getByRole('button', { name: /^send 1 dish$/i }))

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/visits/{id}/rounds', {
        params: { path: { id: VISIT_ID } },
        body: {
          clientKey: expect.any(String) as string,
          lines: [{ dishId: SOUP, quantity: 1, note: 'no onions' }],
        },
      })
    })
  }) // covers: AC-6, AC-7 (spec 0011)

  it('sends the same key again after a failure, and clears once a send lands', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: { error: 'unavailable', message: 'timed out' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Tomato soup' }))
    await user.click(screen.getByRole('button', { name: /^send 1 dish$/i }))
    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledTimes(1)
    })

    // The retry lands, as the replay of the first try would.
    vi.mocked(api.POST).mockResolvedValueOnce({ data: {} })
    await user.click(screen.getByRole('button', { name: /^send 1 dish$/i }))
    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledTimes(2)
    })

    const keys = vi
      .mocked(api.POST)
      .mock.calls.map((call) => JSON.stringify(call[1]).match(/"clientKey":"([^"]+)"/)?.[1])
    expect(keys[0]).toBeDefined()
    expect(keys[1]).toBe(keys[0])

    await waitFor(() => {
      expect(window.sessionStorage.getItem(`waiter.basket.${VISIT_ID}`)).toBeNull()
    })
  }) // covers: AC-7, AC-8 (spec 0011)

  it('drops the basket, and says why, when the table has closed', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'visit_not_open', message: 'that party has already left' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Tomato soup' }))
    await user.click(screen.getByRole('button', { name: /^send 1 dish$/i }))

    await waitFor(() => {
      expect(window.sessionStorage.getItem(`waiter.basket.${VISIT_ID}`)).toBeNull()
    })
    expect(screen.queryByRole('textbox', { name: 'Note for Tomato soup' })).not.toBeInTheDocument()
  }) // covers: AC-7 (spec 0011)

  it('refuses to send a note over 140 characters', async () => {
    const user = userEvent.setup()
    respondWith(visitWith('served'), menuWith())
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add one Tomato soup' }))
    await user.click(screen.getByRole('textbox', { name: 'Note for Tomato soup' }))
    await user.paste('x'.repeat(141))

    expect(await screen.findByText('A note can be at most 140 characters.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^send 1 dish$/i })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
  }) // covers: AC-6 (spec 0011)
})
