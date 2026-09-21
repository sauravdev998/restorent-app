import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { ToastViewport } from '@/shared/ui/toast'
import { expectAccessible } from '@/test/axe'
import { resetMineOnlyForTests } from '@/waiter/mine-only'

import { WaiterOrders } from './orders'

/**
 * The waiter's Orders list (spec 0011, AC-2, AC-3, AC-4, AC-12).
 *
 * Every open table, in the order a waiter should walk it, with every round's
 * status and a way to carry a ready dish out without opening the table.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const ME = '00000000-0000-7000-8000-000000000001'
const COLLEAGUE = '00000000-0000-7000-8000-000000000003'

const IDENTITY: Identity = {
  staff: {
    id: ME,
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

type Status = 'queued' | 'ready' | 'served'

function visit(
  id: string,
  label: string,
  options: {
    responsible?: string
    openedAt?: string
    sentAt?: string
    dishes?: { id: string; name: string; status: Status; readyAt?: string; note?: string }[]
  } = {},
) {
  const dishes = options.dishes ?? []
  return {
    id,
    tableId: `${id}-table`,
    tableLabel: label,
    openedAt: options.openedAt ?? '2026-09-18T18:00:00.000Z',
    responsibleStaffId: options.responsible ?? ME,
    responsibleName: options.responsible === COLLEAGUE ? 'Cara Colleague' : 'Wes Waiter',
    rounds:
      dishes.length === 0
        ? []
        : [
            {
              id: `${id}-round`,
              visitId: id,
              sequenceNo: 1,
              status: dishes.every((dish) => dish.status === 'ready') ? 'ready' : 'queued',
              sentAt: options.sentAt ?? '2026-09-18T18:01:00.000Z',
              readyAt: null,
              servedAt: null,
              lines: dishes.map((dish) => ({
                id: dish.id,
                dishId: `${dish.id}-dish`,
                dishName: dish.name,
                quantity: 1,
                unitPrice: '100.0000',
                lineTotal: '100.0000',
                note: dish.note ?? null,
                status: dish.status,
                readyAt: dish.readyAt ?? null,
                voidReasonCode: null,
                voidReason: null,
              })),
            },
          ],
  }
}

function respondWith(visits: unknown[]) {
  vi.mocked(api.GET).mockImplementation((path: string) => {
    if (path === '/api/me') return Promise.resolve({ data: IDENTITY }) as never
    return Promise.resolve({ data: { visits, serverTime: '2026-09-18T18:10:00.000Z' } }) as never
  })
}

function router() {
  return createMemoryRouter(
    [
      {
        id: 'root',
        path: '/waiter/orders',
        loader: () => IDENTITY,
        element: (
          <>
            <WaiterOrders />
            <ToastViewport />
          </>
        ),
      },
    ],
    { initialEntries: ['/waiter/orders'] },
  )
}

async function mount() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router()} />
    </QueryClientProvider>,
  )

  await act(async () => {
    await Promise.resolve()
  })
}

/** The table headings, top to bottom. */
function tableOrder(): string[] {
  return screen
    .getAllByRole('heading', { level: 2 })
    .map((heading) => heading.textContent)
    .filter((text): text is string => text !== null)
}

beforeEach(() => {
  vi.clearAllMocks()
  window.localStorage.clear()
  resetMineOnlyForTests()
})

describe('WaiterOrders', () => {
  it('puts ready food first, then cooking, then the rest', async () => {
    respondWith([
      visit('idle', '1', { openedAt: '2026-09-18T17:00:00.000Z' }),
      visit('cooking', '2', { dishes: [{ id: 'c1', name: 'Dal', status: 'queued' }] }),
      visit('ready', '3', {
        dishes: [{ id: 'r1', name: 'Soup', status: 'ready', readyAt: '2026-09-18T18:05:00.000Z' }],
      }),
    ])
    await mount()

    await screen.findByText('Table 3')
    expect(tableOrder()).toEqual(['Table 3', 'Table 2', 'Table 1'])
    expect(screen.getByText('1 ready')).toBeInTheDocument()
  }) // covers: AC-2 (spec 0011)

  it('serves one ready dish from the list without opening the table', async () => {
    const user = userEvent.setup()
    respondWith([
      visit('ready', '3', {
        dishes: [
          { id: 'r1', name: 'Soup', status: 'ready', readyAt: '2026-09-18T18:05:00.000Z' },
          { id: 'r2', name: 'Dal', status: 'queued' },
        ],
      }),
    ])
    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Serve Soup' }))

    expect(api.POST).toHaveBeenCalledWith('/api/order-lines/{id}/served', {
      params: { path: { id: 'r1' } },
    })
    // Only the ready dish can be served; the cooking one offers nothing.
    expect(screen.queryByRole('button', { name: 'Serve Dal' })).not.toBeInTheDocument()
  }) // covers: AC-12 (spec 0011)

  it('shows each note whole', async () => {
    respondWith([
      visit('cooking', '2', {
        dishes: [{ id: 'c1', name: 'Dal', status: 'queued', note: 'no onions, nut allergy' }],
      }),
    ])
    await mount()

    expect(await screen.findByText('no onions, nut allergy')).toBeInTheDocument()
  }) // covers: AC-6 (spec 0011)

  it('names who is responsible, and offers to take over only somebody else’s table', async () => {
    const user = userEvent.setup()
    respondWith([
      visit('mine', '1', { openedAt: '2026-09-18T17:00:00.000Z' }),
      visit('theirs', '2', { responsible: COLLEAGUE }),
    ])
    vi.mocked(api.POST).mockResolvedValue({ data: {} })
    await mount()

    const mine = (await screen.findByText('Table 1')).closest('li')
    const theirs = screen.getByText('Table 2').closest('li')
    if (!mine || !theirs) throw new Error('no cards')

    expect(within(mine).getByText('Your table')).toBeInTheDocument()
    expect(within(mine).queryByRole('button', { name: 'Take over' })).not.toBeInTheDocument()
    expect(within(theirs).getByText("Cara Colleague's table")).toBeInTheDocument()

    await user.click(within(theirs).getByRole('button', { name: 'Take over' }))

    expect(api.POST).toHaveBeenCalledWith('/api/visits/{id}/take-over', {
      params: { path: { id: 'theirs' } },
      body: { expectedStaffId: COLLEAGUE },
    })
  }) // covers: AC-4 (spec 0011)

  it('tells a waiter who lost a take over what happened', async () => {
    const user = userEvent.setup()
    respondWith([visit('theirs', '2', { responsible: COLLEAGUE })])
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'table_taken_over', message: 'somebody else took that table over first' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Take over' }))

    expect(
      await screen.findByText(
        'Somebody else took that table over first. The screen now shows who has it.',
      ),
    ).toBeInTheDocument()
  }) // covers: AC-4, AC-17 (spec 0011)

  it('narrows to my tables with Mine', async () => {
    const user = userEvent.setup()
    respondWith([visit('mine', '1'), visit('theirs', '2', { responsible: COLLEAGUE })])
    await mount()

    await screen.findByText('Table 2')
    await user.click(screen.getByRole('switch', { name: 'Show only my tables' }))

    await waitFor(() => {
      expect(screen.queryByText('Table 2')).not.toBeInTheDocument()
    })
    expect(screen.getByText('Table 1')).toBeInTheDocument()
  }) // covers: AC-3 (spec 0011)

  it('says there are no open tables rather than showing a blank list', async () => {
    respondWith([])
    await mount()

    expect(await screen.findByText('No open tables')).toBeInTheDocument()
  }) // covers: AC-2 (spec 0011)

  it('is accessible in both appearances and at every density', async () => {
    respondWith([
      visit('ready', '3', {
        dishes: [{ id: 'r1', name: 'Soup', status: 'ready', readyAt: '2026-09-18T18:05:00.000Z' }],
      }),
      visit('theirs', '2', { responsible: COLLEAGUE }),
    ])

    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

    await expectAccessible(
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router()} />
      </QueryClientProvider>,
    )
  }) // covers: AC-19 (spec 0011)
})
