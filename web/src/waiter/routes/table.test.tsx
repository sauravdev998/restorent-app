import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { menuKey } from '@/shared/events/query-keys'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { expectAccessible } from '@/test/axe'

import { WaiterTable } from './table'

/**
 * The waiter's table screen, and the one rule on it that is easy to get wrong.
 *
 * **A round is announced once.** The screen refetches every time the live
 * stream says anything on this visit moved, and a round that is ready stays
 * ready across all of those refetches. Without the guard, every refetch would
 * raise the alert again: a waiter carrying three plates would have their phone
 * shouting at them about food they collected two minutes ago, and they would
 * learn to ignore it, which is the one outcome this alert cannot survive.
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
  },
}

const VISIT_ID = '00000000-0000-7000-8000-000000000010'
const ROUND_ID = '00000000-0000-7000-8000-000000000020'

/** A visit document, with its one round in whatever state a test needs. */
function visitWith(roundStatus: 'queued' | 'ready' | 'served') {
  return {
    id: VISIT_ID,
    tableId: '00000000-0000-7000-8000-000000000011',
    tableLabel: '7',
    status: 'open',
    guestCount: 2,
    openedBy: 'Wes Waiter',
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
            id: '00000000-0000-7000-8000-000000000030',
            dishId: '00000000-0000-7000-8000-000000000040',
            dishName: 'Tomato soup',
            quantity: 1,
            unitPrice: '180.0000',
            lineTotal: '180.0000',
            note: null,
            status: roundStatus === 'served' ? 'served' : roundStatus,
          },
        ],
      },
    ],
    bill: {
      id: '00000000-0000-7000-8000-000000000050',
      number: null,
      status: 'open',
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
})

describe('WaiterTable', () => {
  it('says nothing about food being ready while it is still cooking', async () => {
    respondWith(visitWith('queued'))
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7')).toBeInTheDocument()
    })

    expect(screen.queryByText(/is ready/i)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /mark served/i })).not.toBeInTheDocument()
  }) // covers: AC-8, AC-10

  it('raises the alert naming the table and the round when food is ready', async () => {
    respondWith(visitWith('ready'))
    await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7, round 1 is ready')).toBeInTheDocument()
    })

    // And the alert carries the action, so collecting the food and saying so
    // are one tap rather than two screens.
    expect(screen.getByRole('button', { name: /mark served/i })).toBeInTheDocument()
  }) // covers: AC-8

  it('announces the same round once, however many times the screen refetches', async () => {
    // The rule this whole file exists for. Every live event on this visit
    // refetches the document, and a ready round stays ready in all of them.
    respondWith(visitWith('ready'))
    const { queryClient } = await mount()

    await waitFor(() => {
      expect(screen.getByText('Table 7, round 1 is ready')).toBeInTheDocument()
    })

    // Dismissed, the way a waiter who has collected the food would.
    screen.getByRole('button', { name: /dismiss/i }).click()

    await waitFor(() => {
      expect(screen.queryByText('Table 7, round 1 is ready')).not.toBeInTheDocument()
    })

    // Three more refetches of the very same answer, which is what a busy table
    // produces: a line moving, a bill changing, another round being sent.
    for (let i = 0; i < 3; i++) {
      await act(async () => {
        await queryClient.refetchQueries({ queryKey: ['visit', VISIT_ID] })
      })
    }

    expect(
      screen.queryByText('Table 7, round 1 is ready'),
      'the alert came back on a refetch, so a waiter is shouted at about food they already have',
    ).not.toBeInTheDocument()
  }) // covers: AC-8

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
