import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { floorKey } from '@/shared/events/query-keys'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { ToastViewport } from '@/shared/ui/toast'
import { expectAccessible } from '@/test/axe'

import { WaiterFloor } from './floor'

/**
 * The floor, and the rule that makes it safe to have several waiters on it.
 *
 * **Nothing here is written optimistically.** A table is taken when the server
 * says it is. A screen that drew a table as occupied the moment somebody tapped
 * it would show two waiters, on two phones, each believing they had table four,
 * and only one of them would be right. So the tap shows the wait on its own
 * button and the floor changes when the answer comes back.
 *
 * **A refusal is read in the reader's own language.** `table_occupied` arrives
 * as a code; the sentence belongs on this side, with the rest of the words.
 *
 * The client is replaced wholesale, the way the other screen tests do it: the
 * real one builds a `Request` from a relative address, which jsdom refuses.
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

const FREE_TABLE = '00000000-0000-7000-8000-000000000011'
const TAKEN_TABLE = '00000000-0000-7000-8000-000000000012'
const OPEN_VISIT = '00000000-0000-7000-8000-000000000020'

/** One section, one free table and one with a party at it. */
function floor(overrides: { foodReady?: boolean } = {}) {
  return {
    sections: [
      {
        id: '00000000-0000-7000-8000-000000000010',
        name: 'Main room',
        tables: [
          { id: FREE_TABLE, label: '1', seats: 4, occupancy: null },
          {
            id: TAKEN_TABLE,
            label: '2',
            seats: 4,
            occupancy: {
              visitId: OPEN_VISIT,
              openedBy: 'Wes Waiter',
              openedAt: '2026-09-08T12:00:00.000Z',
              guestCount: 2,
              foodReady: overrides.foodReady ?? false,
            },
          },
        ],
      },
    ],
  }
}

function respondWith(data: unknown) {
  vi.mocked(api.GET).mockResolvedValue({ data })
}

/** Where the screen walked to, so a navigation can be asserted without a real one. */
function routerFor(element: React.ReactElement) {
  return createMemoryRouter(
    [
      { id: 'root', path: '/waiter', element, loader: () => IDENTITY },
      { path: '/waiter/tables/:visitId', element: <p>table screen for the visit</p> },
    ],
    { initialEntries: ['/waiter'] },
  )
}

async function mount() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider
        router={routerFor(
          <>
            <WaiterFloor />
            <ToastViewport />
          </>,
        )}
      />
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

describe('WaiterFloor', () => {
  it('lists every table under its section, in the order the server sent them', async () => {
    // The server walks the room in section then position order. This screen
    // must not re-sort, or the floor would come back alphabetical and stop
    // matching the room a waiter is standing in.
    respondWith(floor())
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Main room' })).toBeInTheDocument()
    })

    const labels = screen.getAllByText(/^[12]$/).map((node) => node.textContent)
    expect(labels).toEqual(['1', '2'])
  }) // covers: AC-1

  it('names who opened a taken table and offers a free one', async () => {
    respondWith(floor())
    await mount()

    await waitFor(() => {
      expect(screen.getByText(/Opened by Wes Waiter at/)).toBeInTheDocument()
    })

    expect(screen.getByText('Free')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Open table' })).toBeInTheDocument()

    // Any waiter may still act on a table somebody else opened. Naming the
    // opener is a fact worth knowing at a shift change, not a lock.
    expect(screen.getByRole('button', { name: 'View' })).toBeInTheDocument()
  }) // covers: AC-1

  it('marks a table whose food is waiting to be collected', async () => {
    respondWith(floor({ foodReady: true }))
    await mount()

    await waitFor(() => {
      expect(screen.getByText(/Opened by Wes Waiter at/)).toBeInTheDocument()
    })

    expect(screen.getByText('Ready')).toBeInTheDocument()
  }) // covers: AC-1, AC-8

  it('shows no ready marker on a table whose food is still cooking', async () => {
    // The other half of the same rule. A marker that was always on would tell
    // a waiter nothing, and they would stop looking at it.
    respondWith(floor({ foodReady: false }))
    await mount()

    await waitFor(() => {
      expect(screen.getByText(/Opened by Wes Waiter at/)).toBeInTheDocument()
    })

    expect(screen.queryByText('Ready')).not.toBeInTheDocument()
  }) // covers: AC-1

  it('opens a free table and walks straight into taking the order', async () => {
    // A waiter is not opening a table for its own sake, they are taking an
    // order, so the two are one tap.
    respondWith(floor())
    vi.mocked(api.POST).mockResolvedValue({ data: { visitId: OPEN_VISIT } })

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Open table' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Open table' }))

    await waitFor(() => {
      expect(screen.getByText('table screen for the visit')).toBeInTheDocument()
    })

    expect(api.POST).toHaveBeenCalledWith('/api/visits', { body: { tableId: FREE_TABLE } })
  }) // covers: AC-1

  it('walks to a taken table without writing anything', async () => {
    respondWith(floor())

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'View' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'View' }))

    await waitFor(() => {
      expect(screen.getByText('table screen for the visit')).toBeInTheDocument()
    })

    // Viewing is a read. Opening a table that already has a party would be a
    // second visit on one table, which is the thing the whole slice guards.
    expect(api.POST).not.toHaveBeenCalled()
  }) // covers: AC-1, AC-2

  it('shows the wait on the tapped table without drawing it as taken', async () => {
    // No cache optimism. Until the server answers, the table is still free as
    // far as this screen knows, and it says so.
    respondWith(floor())

    let settle: (() => void) | undefined
    vi.mocked(api.POST).mockReturnValue(
      new Promise((resolve) => {
        settle = () => {
          resolve({ data: { visitId: OPEN_VISIT } })
        }
      }),
    )

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Open table' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Open table' }))

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Opening' })).toHaveAttribute(
        'aria-disabled',
        'true',
      )
    })

    // Still shown as free, because nothing has said otherwise yet.
    expect(screen.getByText('Free')).toBeInTheDocument()

    await act(async () => {
      settle?.()
      await Promise.resolve()
    })
  }) // covers: AC-2

  it('tells a waiter who lost the table what happened, in words they can read', async () => {
    respondWith(floor())
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'table_occupied', message: 'that table already has a party at it' },
    })

    const user = userEvent.setup()
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Open table' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Open table' }))

    await waitFor(() => {
      expect(
        screen.getByText(
          'Somebody has already opened that table. Its screen now shows where it stands.',
        ),
      ).toBeInTheDocument()
    })

    expect(
      screen.queryByText('that table already has a party at it'),
      'the API’s English reached a waiter’s screen',
    ).not.toBeInTheDocument()
  }) // covers: AC-2, AC-13

  it('refetches the floor after a refusal, so the screen shows where things stand', async () => {
    respondWith(floor())
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'table_occupied', message: 'already taken' },
    })

    const user = userEvent.setup()
    const { queryClient } = await mount()
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Open table' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Open table' }))

    await waitFor(() => {
      expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: floorKey })
    })
  }) // covers: AC-2, AC-13

  it('puts a table with no section under a heading of its own', async () => {
    // A table can belong to no section, or to one since archived. It is still a
    // table, and it must not vanish off the floor.
    respondWith({
      sections: [
        {
          id: null,
          name: null,
          tables: [{ id: FREE_TABLE, label: '9', seats: 2, occupancy: null }],
        },
      ],
    })
    await mount()

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Other tables' })).toBeInTheDocument()
    })

    expect(screen.getByText('9')).toBeInTheDocument()
  }) // covers: AC-1

  it('says the restaurant has no tables rather than showing a blank floor', async () => {
    respondWith({ sections: [{ id: null, name: null, tables: [] }] })
    await mount()

    await waitFor(() => {
      expect(screen.getByText('No tables yet')).toBeInTheDocument()
    })
  }) // covers: AC-1

  it('is accessible in both appearances and at every density', async () => {
    respondWith(floor({ foodReady: true }))

    await expectAccessible(
      <QueryClientProvider
        client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
      >
        <RouterProvider router={routerFor(<WaiterFloor />)} />
      </QueryClientProvider>,
    )
  }) // covers: AC-19
})
