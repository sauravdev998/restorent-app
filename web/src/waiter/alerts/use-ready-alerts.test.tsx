import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { openOrdersKey } from '@/shared/events/query-keys'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { subscribeToAnnouncements } from '@/shared/ui/announce'
import { playReadyChime } from '@/shared/ui/audio-unlock'
import { expectAccessible } from '@/test/axe'
import { WaiterLayout } from '@/waiter/routes/waiter-layout'

import { resetAcknowledgedForTests } from './acknowledged'
import { REMINDER_MS } from './ready'

/**
 * The waiter's ready alert, mounted in the waiter shell (spec 0011, AC-9,
 * AC-10, AC-11).
 *
 * The chime is stood in for, because jsdom has no audio. What is under test is
 * when this phone chimes, what it shows, and when it stops: once per dish, only
 * for my tables, never on load, every two minutes until acknowledged.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

vi.mock('@/shared/ui/audio-unlock', () => ({
  playReadyChime: vi.fn(),
  primeAudioUnlock: vi.fn(() => () => undefined),
  isAudioUnlocked: vi.fn(() => true),
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
  },
}

type Status = 'queued' | 'ready' | 'served'

/** One open table with one round, each dish in the state a test names. */
function table(
  visitId: string,
  label: string,
  responsible: string,
  dishes: { id: string; name: string; status: Status }[],
) {
  return {
    id: visitId,
    tableId: `${visitId}-table`,
    tableLabel: label,
    openedAt: '2026-09-18T18:00:00.000Z',
    responsibleStaffId: responsible,
    responsibleName: responsible === ME ? 'Wes Waiter' : 'Cara Colleague',
    rounds: [
      {
        id: `${visitId}-round`,
        visitId,
        sequenceNo: 1,
        status: 'queued',
        sentAt: '2026-09-18T18:01:00.000Z',
        readyAt: null,
        servedAt: null,
        lines: dishes.map((dish) => ({
          id: dish.id,
          dishId: `${dish.id}-dish`,
          dishName: dish.name,
          quantity: 1,
          unitPrice: '100.0000',
          lineTotal: '100.0000',
          note: null,
          status: dish.status,
          readyAt: dish.status === 'queued' ? null : '2026-09-18T18:05:00.000Z',
          voidReasonCode: null,
          voidReason: null,
        })),
      },
    ],
  }
}

let visits: unknown[] = []

function answer() {
  vi.mocked(api.GET).mockImplementation((path: string) => {
    if (path === '/api/me') return Promise.resolve({ data: IDENTITY }) as never
    return Promise.resolve({
      data: { visits, serverTime: '2026-09-18T18:06:00.000Z' },
    }) as never
  })
}

async function mount() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

  const router = createMemoryRouter(
    [
      {
        id: 'root',
        path: '/waiter',
        loader: () => IDENTITY,
        element: <WaiterLayout />,
        children: [{ index: true, element: <p>the floor</p> }],
      },
    ],
    { initialEntries: ['/waiter'] },
  )

  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  await screen.findByText('the floor')
  // The first answer has to land before a test changes it, or the change is
  // what the phone sees first, and that is rightly treated as food that was
  // already ready on load.
  await waitFor(() => {
    expect(queryClient.getQueryState(openOrdersKey)?.status).toBe('success')
  })
  await act(async () => {
    await Promise.resolve()
  })

  return {
    refetch: async () => {
      await act(async () => {
        await queryClient.invalidateQueries({ queryKey: openOrdersKey })
      })
    },
  }
}

const said: string[] = []
let stopListening: () => void = () => undefined

beforeEach(() => {
  vi.clearAllMocks()
  window.sessionStorage.clear()
  resetAcknowledgedForTests()
  visits = []
  said.length = 0
  stopListening = subscribeToAnnouncements((message) => {
    said.push(message)
  })
  answer()
})

afterEach(() => {
  stopListening()
  vi.useRealTimers()
})

describe('the ready alert', () => {
  it('shows food that was already ready on load, without a chime', async () => {
    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'ready' }])]
    await mount()

    expect(await screen.findByRole('heading', { name: '1 dish ready to collect' })).toBeVisible()
    expect(playReadyChime).not.toHaveBeenCalled()
  }) // covers: AC-11 (spec 0011)

  it('chimes, vibrates, and announces once when a dish on my table turns ready', async () => {
    const vibrate = vi.fn(() => true)
    Object.defineProperty(navigator, 'vibrate', { value: vibrate, configurable: true })

    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'queued' }])]
    const { refetch } = await mount()
    expect(playReadyChime).not.toHaveBeenCalled()

    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'ready' }])]
    await refetch()

    await waitFor(() => {
      expect(playReadyChime).toHaveBeenCalledTimes(1)
    })
    expect(vibrate).toHaveBeenCalledWith([200, 100, 200])
    expect(said).toContain('Ready at table 4: 1 × Tomato soup.')
    expect(screen.getByRole('link', { name: 'Table 4' })).toHaveAttribute(
      'href',
      '/waiter/tables/v1',
    )

    // The same answer again, as every live event produces: no second chime.
    await refetch()
    await refetch()
    expect(playReadyChime).toHaveBeenCalledTimes(1)
  }) // covers: AC-9 (spec 0011)

  it('stays silent and shows nothing for food on somebody else’s table', async () => {
    visits = [table('v2', '9', COLLEAGUE, [{ id: 'l2', name: 'Naan', status: 'queued' }])]
    const { refetch } = await mount()

    visits = [table('v2', '9', COLLEAGUE, [{ id: 'l2', name: 'Naan', status: 'ready' }])]
    await refetch()

    expect(playReadyChime).not.toHaveBeenCalled()
    expect(screen.queryByRole('heading', { name: /ready to collect/ })).not.toBeInTheDocument()
  }) // covers: AC-9 (spec 0011)

  it('reminds every two minutes until acknowledged, then stops', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'ready' }])]
    await mount()
    await screen.findByRole('heading', { name: '1 dish ready to collect' })

    act(() => {
      vi.advanceTimersByTime(REMINDER_MS)
    })
    expect(playReadyChime).toHaveBeenCalledTimes(1)

    act(() => {
      vi.advanceTimersByTime(REMINDER_MS)
    })
    expect(playReadyChime).toHaveBeenCalledTimes(2)

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) })
    await user.click(screen.getByRole('button', { name: 'Acknowledge' }))

    await waitFor(() => {
      expect(screen.queryByRole('heading', { name: /ready to collect/ })).not.toBeInTheDocument()
    })
    act(() => {
      vi.advanceTimersByTime(REMINDER_MS * 3)
    })
    expect(playReadyChime).toHaveBeenCalledTimes(2)
    expect(window.sessionStorage.getItem('waiter.acknowledged')).toContain('l1')
  }) // covers: AC-10 (spec 0011)

  it('stops reminding once the dish is served', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'ready' }])]
    const { refetch } = await mount()
    await screen.findByRole('heading', { name: '1 dish ready to collect' })

    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'served' }])]
    await refetch()
    await waitFor(() => {
      expect(screen.queryByRole('heading', { name: /ready to collect/ })).not.toBeInTheDocument()
    })

    act(() => {
      vi.advanceTimersByTime(REMINDER_MS * 2)
    })
    expect(playReadyChime).not.toHaveBeenCalled()
  }) // covers: AC-10 (spec 0011)

  it('is accessible while it is showing', async () => {
    visits = [table('v1', '4', ME, [{ id: 'l1', name: 'Tomato soup', status: 'ready' }])]

    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

    await expectAccessible(
      <QueryClientProvider client={queryClient}>
        <RouterProvider
          router={createMemoryRouter(
            [
              {
                id: 'root',
                path: '/waiter',
                loader: () => IDENTITY,
                element: <WaiterLayout />,
                children: [{ index: true, element: <h1>the floor</h1> }],
              },
            ],
            { initialEntries: ['/waiter'] },
          )}
        />
      </QueryClientProvider>,
    )
  }) // covers: AC-19 (spec 0011)
})
