import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { MemoryRouter } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { menuKey } from '@/shared/events/query-keys'
import { expectAccessible } from '@/test/axe'

import { KitchenMenu } from './kitchen-menu'

/**
 * The kitchen's Menu tab, spec 0008 AC-10.
 *
 * A chef's whole menu power is one switch per dish, so that is what is under
 * test: every live dish grouped by category, a switch per dish that says what
 * it is for, no price anywhere, and nothing else a chef could change.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PUT: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const SOUP = '00000000-0000-7000-8000-000000000020'

const MENU = {
  currencyCode: 'INR',
  currencyDecimals: 2,
  categories: [
    {
      id: '00000000-0000-7000-8000-000000000010',
      name: 'Starters',
      dishes: [
        {
          id: SOUP,
          name: 'Tomato soup',
          description: null,
          price: '180.0000',
          diet: 'veg',
          available: true,
        },
        {
          id: '00000000-0000-7000-8000-000000000021',
          name: 'Fish pakora',
          description: null,
          price: '290.0000',
          diet: 'non_veg',
          available: false,
        },
      ],
    },
  ],
}

function wrap(ui: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return {
    queryClient,
    ui: (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={['/kitchen/menu']}>{ui}</MemoryRouter>
      </QueryClientProvider>
    ),
  }
}

async function mount() {
  const { ui } = wrap(<KitchenMenu />)
  render(ui)
  await act(async () => {
    await Promise.resolve()
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(api.GET).mockResolvedValue({ data: MENU })
})

describe('KitchenMenu', () => {
  it('lists every live dish by category with a switch saying whether it is on', async () => {
    await mount()

    const starters = await screen.findByRole('region', { name: 'Starters' })
    expect(within(starters).getByRole('switch', { name: 'Tomato soup available' })).toHaveAttribute(
      'aria-checked',
      'true',
    )
    expect(within(starters).getByRole('switch', { name: 'Fish pakora available' })).toHaveAttribute(
      'aria-checked',
      'false',
    )
    expect(within(starters).getByText('Off')).toBeInTheDocument()
  }) // covers: AC-10 (spec 0008)

  it('shows no price and offers nothing but the switch', async () => {
    await mount()

    const starters = await screen.findByRole('region', { name: 'Starters' })
    expect(within(starters).queryByText(/₹/)).not.toBeInTheDocument()
    expect(within(starters).queryAllByRole('button')).toHaveLength(0)
  }) // covers: AC-10 (spec 0008)

  it('sits beside the pass, with the Menu tab marked as the one on screen', async () => {
    await mount()

    const tabs = screen.getByRole('navigation', { name: 'Kitchen screens' })
    expect(within(tabs).getByRole('link', { name: 'Menu' })).toHaveAttribute('aria-current', 'page')
    expect(within(tabs).getByRole('link', { name: 'The pass' })).not.toHaveAttribute('aria-current')
  }) // covers: AC-10 (spec 0008)

  it('switches a dish off with the absolute value the chef asked for', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValue({ data: {} } as never)
    await mount()

    await user.click(await screen.findByRole('switch', { name: 'Tomato soup available' }))

    expect(api.PUT).toHaveBeenCalledWith('/api/dishes/{id}/availability', {
      params: { path: { id: SOUP } },
      body: { available: false },
    })
  }) // covers: AC-9 (spec 0008)

  it('is accessible in both appearances and at every density', async () => {
    const { queryClient, ui } = wrap(
      <main>
        <KitchenMenu />
      </main>,
    )
    queryClient.setQueryData(menuKey, MENU)

    await expectAccessible(ui)
  }) // covers: AC-10, AC-19 (spec 0008)
})
