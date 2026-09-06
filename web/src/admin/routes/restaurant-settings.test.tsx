import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { api } from '@/shared/api/client'
import { expectAccessible } from '@/test/axe'

import { RestaurantSettings } from './restaurant-settings'

/**
 * The five things an admin can change about their restaurant, and the two they
 * cannot.
 *
 * The client is replaced wholesale, for the reason the other screen tests give:
 * the real one builds a `Request` from a relative address, which jsdom refuses.
 * What is under test is what this screen does with an answer.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const IDENTITY: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Ada Owner',
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

async function mount(identity: Identity = IDENTITY) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, identity)

  const router = createMemoryRouter(
    [{ id: 'root', path: '/', element: <RestaurantSettings />, loader: () => identity }],
    { initialEntries: ['/'] },
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

describe('RestaurantSettings', () => {
  it('is accessible, headings and labelled controls included', async () => {
    await expectAccessible(
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider
          router={createMemoryRouter(
            [{ id: 'root', path: '/', element: <RestaurantSettings />, loader: () => IDENTITY }],
            { initialEntries: ['/'] },
          )}
        />
      </QueryClientProvider>,
    )
  }) // covers: AC-21

  it('offers the five settings that can change and no currency box', async () => {
    // The currency and its decimals were fixed by the country at registration
    // and money already written in them cannot be reinterpreted, so this screen
    // must not offer a control that looks like it could. It says what the
    // currency is instead.
    await mount()

    expect(screen.getByLabelText(/Restaurant name/)).toHaveValue('The Test Kitchen')
    expect(screen.getByLabelText(/Address/)).toHaveValue('')
    expect(screen.getByLabelText(/Timezone/)).toHaveValue('Asia/Kolkata')
    expect(screen.getByRole('combobox', { name: /Restaurant language/ })).toHaveValue('en')
    expect(screen.getByRole('combobox', { name: /Number and date format/ })).toHaveValue('en-IN')

    const controls = screen.getAllByRole('textbox').concat(screen.getAllByRole('combobox'))
    expect(controls).toHaveLength(5)
    expect(screen.queryByLabelText(/Currency/)).not.toBeInTheDocument()
    expect(screen.getByText(/Prices are in INR/)).toBeInTheDocument()
  }) // covers: AC-15

  it('puts a refused timezone beside the timezone box and nowhere else', async () => {
    // A zone nobody has is the one refusal this form has to place well: told
    // about it anywhere but on that box, an admin has five fields to guess
    // between and four of them were fine.
    vi.mocked(api.PATCH).mockResolvedValue({
      error: {
        error: 'invalid',
        message: 'no',
        fields: { timezone: 'not_in_catalogue' },
      },
      response: new Response(null, { status: 400 }),
    })

    await mount()

    await userEvent.clear(screen.getByLabelText(/Timezone/))
    await userEvent.type(screen.getByLabelText(/Timezone/), 'Not/AZone')
    await userEvent.click(screen.getByRole('button', { name: 'Save settings' }))

    await waitFor(() => {
      expect(screen.getByLabelText(/Timezone/)).toHaveAttribute('aria-invalid', 'true')
    })
    expect(screen.getByLabelText(/Restaurant name/)).not.toHaveAttribute('aria-invalid')

    // And the box keeps what was typed, so the admin can see what was refused
    // rather than an empty field and a message about it.
    expect(screen.getByLabelText(/Timezone/)).toHaveValue('Not/AZone')
  }) // covers: AC-15

  it('sends all five fields together, so a blank address is a real value', async () => {
    // A `PATCH` that left out the untouched fields would make "I cleared the
    // address" indistinguishable from "I did not touch the address".
    vi.mocked(api.PATCH).mockResolvedValue({
      data: { ...IDENTITY },
      response: new Response(null, { status: 200 }),
    })

    await mount()

    await userEvent.type(screen.getByLabelText(/Address/), '12 Marine Drive')
    await userEvent.click(screen.getByRole('button', { name: 'Save settings' }))

    await waitFor(() => {
      expect(vi.mocked(api.PATCH)).toHaveBeenCalledWith('/api/restaurant', {
        body: {
          name: 'The Test Kitchen',
          address: '12 Marine Drive',
          timezone: 'Asia/Kolkata',
          defaultLanguage: 'en',
          formattingLocale: 'en-IN',
        },
      })
    })
  }) // covers: AC-15

  it('says the currency of the restaurant it is showing now, not the one it loaded', async () => {
    // The regression the identity hook exists for, on this screen. It used to
    // read the route loader's snapshot, which React Router takes once and never
    // revisits, so the bundle this very form's save writes back reached the
    // cache and not the screen.
    const { queryClient } = await mount()
    expect(screen.getByText(/Prices are in INR/)).toBeInTheDocument()

    const moved: Identity = {
      ...IDENTITY,
      restaurant: { ...IDENTITY.restaurant, currencyCode: 'GBP', countryCode: 'GB' },
    }

    act(() => {
      queryClient.setQueryData<Identity | null>(IDENTITY_KEY, moved)
    })

    await waitFor(() => {
      expect(screen.getByText(/Prices are in GBP/)).toBeInTheDocument()
    })
  }) // covers: AC-15
})
