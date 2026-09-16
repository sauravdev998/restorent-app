import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { describe, expect, it } from 'vitest'

import { IDENTITY_KEY, type Identity } from './identity'
import { useIdentity } from './use-identity'

/**
 * The hook every signed in screen reads its identity through.
 *
 * Two sources answer the same question, which is the whole reason this exists.
 * The root route's loader resolves an identity before any protected screen
 * renders, and React Router hands that answer back unchanged for as long as the
 * route is mounted. Meanwhile `PATCH /api/me` and `PATCH /api/restaurant` both
 * reply with a fresh bundle and write it into the identity query. A screen
 * reading only the loader therefore kept the stale one, and changing your own
 * language, or the restaurant's name, did nothing on screen until a reload.
 *
 * So the two cases below are the contract: the loader's answer carries the
 * first render, and the query wins the moment it holds something newer.
 */

const LOADED: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Ada Owner',
    email: 'ada@example.test',
    role: 'admin',
    language: null,
  },
  restaurant: {
    id: '00000000-0000-7000-8000-000000000002',
    name: 'The Loader Kitchen',
    address: null,
    countryCode: 'IN',
    currencyCode: 'INR',
    currencyDecimals: 2,
    timezone: 'Asia/Kolkata',
    defaultLanguage: 'en',
    formattingLocale: 'en-IN',
  },
}

/** A reader, so the assertions are about what a screen would show. */
function Reader() {
  const identity = useIdentity()

  return (
    <dl>
      <dt>restaurant</dt>
      <dd data-testid="restaurant">{identity.restaurant.name}</dd>
      <dt>language</dt>
      <dd data-testid="language">{identity.staff.language ?? 'follows the restaurant'}</dd>
    </dl>
  )
}

/**
 * Mounts the reader the way the real tree does: inside a route named `root`
 * whose loader has resolved an identity.
 *
 * `primed` says whether the query already holds one. The real loader primes it
 * with `ensureQueryData`, so in the app they start out the same object; passing
 * `false` is the honest way to prove which of the two the hook actually falls
 * back on.
 */
async function mount({ primed }: { primed: boolean }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  if (primed) queryClient.setQueryData<Identity | null>(IDENTITY_KEY, LOADED)

  const router = createMemoryRouter(
    [{ id: 'root', path: '/', element: <Reader />, loader: () => LOADED }],
    { initialEntries: ['/'] },
  )

  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  // A route with a loader renders nothing on its first pass, so anything
  // asserted straight after `render` would be looking at an empty document.
  await act(async () => {
    await Promise.resolve()
  })

  return { ...view, queryClient }
}

describe('useIdentity', () => {
  it('falls back to the loader while nothing newer has been written', async () => {
    // The first render of a real navigation: the loader has an answer and
    // nothing has replied with a fresh bundle yet. Without the fallback this
    // is a screen with no identity at all, which is a crash rather than a
    // stale name.
    await mount({ primed: false })

    expect(screen.getByTestId('restaurant')).toHaveTextContent('The Loader Kitchen')
    expect(screen.getByTestId('language')).toHaveTextContent('follows the restaurant')
  }) // covers: AC-16

  it('follows the query once something writes a newer bundle', async () => {
    const { queryClient } = await mount({ primed: true })
    expect(screen.getByTestId('language')).toHaveTextContent('follows the restaurant')

    // Exactly what `PATCH /api/me` does when somebody picks their own
    // language: it writes the answer into this one entry and nothing else.
    const withHindi: Identity = { ...LOADED, staff: { ...LOADED.staff, language: 'hi' } }

    act(() => {
      queryClient.setQueryData<Identity | null>(IDENTITY_KEY, withHindi)
    })

    // Waited for rather than asserted straight away: the query notifies its
    // subscribers in a batch, so the write lands a tick before the render does.
    await waitFor(() => {
      expect(screen.getByTestId('language')).toHaveTextContent('hi')
    })
  }) // covers: AC-16

  it('follows a newer restaurant too, not only a newer person', async () => {
    // The other writer is `PATCH /api/restaurant`. An admin renaming the
    // restaurant on the settings screen has to see it in the header it is
    // shown in, without reloading the page.
    const { queryClient } = await mount({ primed: true })

    const renamed: Identity = {
      ...LOADED,
      restaurant: { ...LOADED.restaurant, name: 'The Renamed Kitchen' },
    }

    act(() => {
      queryClient.setQueryData<Identity | null>(IDENTITY_KEY, renamed)
    })

    await waitFor(() => {
      expect(screen.getByTestId('restaurant')).toHaveTextContent('The Renamed Kitchen')
    })
  }) // covers: AC-15
})
