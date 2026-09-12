import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { adminMenuKey } from '@/shared/events/query-keys'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { ToastViewport } from '@/shared/ui/toast'
import { expectAccessible } from '@/test/axe'

import { AdminMenuScreen } from './admin-menu'

/**
 * The admin's menu screen, spec 0008.
 *
 * Three promises are the subject. **The API is the authority**: every refusal
 * lands beside the box it concerns or inside the dialog, in the reader's
 * language, translated from its code. **A stale form never writes blind**: a
 * refused edit shows the dish as it now is. **Nothing is written into the
 * cache ahead of the server**: a switch shows what was asked for while it
 * saves, and the screen then reads the menu again.
 *
 * The client is replaced wholesale, the way the other screen tests do it.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PUT: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const IDENTITY: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Ada Admin',
    email: 'ada@example.test',
    role: 'admin',
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

const MAINS = '00000000-0000-7000-8000-000000000010'
const DAL = '00000000-0000-7000-8000-000000000020'
const BIRYANI = '00000000-0000-7000-8000-000000000021'

/** One dish as the admin menu carries it. */
function dish(id: string, name: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    categoryId: MAINS,
    name,
    description: null,
    price: '340.0000',
    diet: 'veg',
    available: true,
    version: 1,
    archivedAt: null,
    ...overrides,
  }
}

/** A menu with one category of two dishes, one of them off, and one archived dish. */
function menu(overrides: { dal?: Record<string, unknown> } = {}) {
  return {
    currencyCode: 'INR',
    currencyDecimals: 2,
    categories: [
      {
        id: MAINS,
        name: 'Mains',
        version: 1,
        dishes: [
          dish(DAL, 'Dal makhani', overrides.dal),
          dish(BIRYANI, 'Chicken biryani', {
            diet: 'non_veg',
            available: false,
            price: '480.5000',
          }),
        ],
      },
    ],
    archived: {
      categories: [],
      dishes: [
        {
          id: '00000000-0000-7000-8000-000000000030',
          name: 'Kheer',
          diet: 'veg',
          categoryId: '00000000-0000-7000-8000-000000000011',
          categoryName: 'Desserts',
          categoryLive: false,
          archivedAt: '2026-09-10T10:00:00.000Z',
        },
      ],
    },
  }
}

const EMPTY = {
  currencyCode: 'INR',
  currencyDecimals: 2,
  categories: [],
  archived: { categories: [], dishes: [] },
}

/** What `GET /api/admin/menu` answers, changeable mid test. */
let current: unknown = menu()

function answerMenu(next: unknown) {
  current = next
  vi.mocked(api.GET).mockImplementation(() => Promise.resolve({ data: current }) as never)
}

function wrap(ui: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)
  return {
    queryClient,
    ui: <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  }
}

async function mount() {
  const { queryClient, ui } = wrap(
    <>
      <AdminMenuScreen />
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
  answerMenu(menu())
})

describe('AdminMenuScreen', () => {
  it('lays the menu out as sections of dishes, in the order the server sent', async () => {
    await mount()

    const mains = await screen.findByRole('region', { name: 'Mains' })
    const rows = within(mains).getAllByRole('listitem')
    expect(rows.map((row) => within(row).getByRole('img').getAttribute('aria-label'))).toEqual([
      'Vegetarian',
      'Non vegetarian',
    ])
    expect(within(mains).getByText('₹340.00')).toBeInTheDocument()
    expect(within(mains).getByText('₹480.50')).toBeInTheDocument()

    expect(screen.getByText('1 category · 2 dishes · 1 off tonight')).toBeInTheDocument()
    expect(
      within(mains).getByRole('switch', { name: 'Chicken biryani available' }),
    ).toHaveAttribute('aria-checked', 'false')
  }) // covers: AC-2, AC-13, AC-19 (spec 0008)

  it('shows skeletons while the menu loads', async () => {
    vi.mocked(api.GET).mockImplementation(() => new Promise(() => undefined) as never)
    await mount()

    expect(screen.getByRole('status', { name: '' })).toHaveTextContent('Loading')
  }) // covers: AC-19 (spec 0008)

  it('leads an empty menu to adding its first category', async () => {
    const user = userEvent.setup()
    answerMenu(EMPTY)
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add the first category' }))

    expect(screen.getByRole('dialog', { name: 'Add a category' })).toBeInTheDocument()
  }) // covers: AC-1, AC-19 (spec 0008)

  it('switches a dish off, shows it at once, and reads the menu again rather than writing it', async () => {
    const user = userEvent.setup()
    let settle: (value: unknown) => void = () => undefined
    vi.mocked(api.PUT).mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve
        }) as never,
    )
    await mount()

    const toggle = await screen.findByRole('switch', { name: 'Dal makhani available' })
    await user.click(toggle)

    // Held while it saves, so it does not look broken.
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    expect(api.PUT).toHaveBeenCalledWith('/api/dishes/{id}/availability', {
      params: { path: { id: DAL } },
      body: { available: false },
    })

    const readsBefore = vi.mocked(api.GET).mock.calls.length
    answerMenu(menu({ dal: { available: false, version: 2 } }))
    await act(async () => {
      settle({ data: dish(DAL, 'Dal makhani', { available: false, version: 2 }) })
      await Promise.resolve()
    })

    await waitFor(() => {
      expect(vi.mocked(api.GET).mock.calls.length).toBeGreaterThan(readsBefore)
    })
    expect(toggle).toHaveAttribute('aria-checked', 'false')
  }) // covers: AC-9 (spec 0008)

  it('puts each refused field beside its own box, in words', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: {
        error: 'invalid',
        message: 'One or more fields were not accepted.',
        fields: { name: 'already_taken', price: 'too_many_decimals' },
      },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add a dish' }))
    const form = screen.getByRole('dialog', { name: 'Add a dish' })
    await user.type(within(form).getByRole('textbox', { name: /^name/i }), 'dal makhani')
    await user.type(within(form).getByRole('textbox', { name: /^price/i }), '12.345')
    await user.click(within(form).getByRole('button', { name: 'Add dish' }))

    const name = within(form).getByRole('textbox', { name: /^name/i })
    const price = within(form).getByRole('textbox', { name: /^price/i })
    await waitFor(() => {
      expect(name).toHaveAccessibleDescription(expect.stringContaining('That is already taken.'))
    })
    expect(price).toHaveAccessibleDescription(
      expect.stringContaining('That has more decimal places than your currency uses.'),
    )

    // What was typed went to the API, the price as a plain decimal.
    expect(api.POST).toHaveBeenCalledWith('/api/admin/menu/dishes', {
      body: expect.objectContaining({
        name: 'dal makhani',
        price: '12.345',
        categoryId: MAINS,
      }) as unknown,
    })
  }) // covers: AC-14 (spec 0008)

  it('shows a stale edit the dish as it now is, and saves against that', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValueOnce({
      error: { error: 'dish_changed', message: 'that dish changed after the form was opened' },
    } as never)
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Edit Dal makhani' }))
    const form = screen.getByRole('dialog', { name: 'Edit this dish' })
    const price = within(form).getByRole('textbox', { name: /^price/i })
    expect(price).toHaveValue('340.00')

    // The kitchen switched it off and somebody repriced it meanwhile.
    answerMenu(menu({ dal: { price: '360.0000', version: 3, available: false } }))
    await user.clear(price)
    await user.type(price, '350')
    await user.click(within(form).getByRole('button', { name: 'Save changes' }))

    expect(
      await within(form).findByText(
        'Somebody changed this dish after you opened it. The form now shows where it stands.',
      ),
    ).toBeInTheDocument()
    await waitFor(() => {
      expect(price).toHaveValue('360.00')
    })

    vi.mocked(api.PUT).mockResolvedValueOnce({ data: dish(DAL, 'Dal makhani') } as never)
    await user.click(within(form).getByRole('button', { name: 'Save changes' }))
    await waitFor(() => {
      expect(api.PUT).toHaveBeenLastCalledWith('/api/admin/menu/dishes/{id}', {
        params: { path: { id: DAL } },
        body: expect.objectContaining({ version: 3, price: '360.00' }) as unknown,
      })
    })
  }) // covers: AC-15 (spec 0008)

  it('says inside the dialog why a category with dishes cannot be removed', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'category_not_empty', message: 'that category still has dishes' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Remove the Mains category' }))
    const dialog = screen.getByRole('dialog', { name: 'Remove the Mains category?' })
    expect(within(dialog).getByText(/still has 2 dishes in it/)).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: 'Remove category' }))

    expect(
      await within(dialog).findByText(
        'That category still has dishes in it. Move or remove them first.',
      ),
    ).toHaveAttribute('role', 'alert')
  }) // covers: AC-7 (spec 0008)

  it('lists what was removed, and offers a live category for a dish whose own has gone', async () => {
    const user = userEvent.setup()
    await mount()

    const archived = await screen.findByRole('region', { name: 'Archived' })
    expect(within(archived).getByText('From Desserts, which is removed too')).toBeInTheDocument()

    await user.click(within(archived).getByRole('button', { name: 'Put back Kheer' }))
    const dialog = screen.getByRole('dialog', { name: 'Put back Kheer' })

    expect(within(dialog).getByRole('combobox', { name: 'Put it in' })).toHaveValue(MAINS)
    expect(
      within(dialog).getByText('Its old category, Desserts, is removed too, so choose another.'),
    ).toBeInTheDocument()
  }) // covers: AC-6, AC-8 (spec 0008)

  it('is accessible as a page, in both appearances and at every density', async () => {
    const { queryClient, ui } = wrap(
      <main>
        <AdminMenuScreen />
      </main>,
    )
    // In the cache already, so axe scans the loaded screen with its switches,
    // handles, and marks, and not the skeleton that stands in for them.
    queryClient.setQueryData(adminMenuKey, menu())

    await expectAccessible(ui)
  }) // covers: AC-19 (spec 0008)
})
