import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, waitFor } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, describe, expect, it } from 'vitest'

import { changeLanguage } from '@/shared/i18n'
import { FALLBACK_LANGUAGE } from '@/shared/i18n/catalogue'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { DEFAULT_SURFACE } from '@/shared/surface'
import { expectAccessible } from '@/test/axe'

import { RootLayout } from './root-layout'

/**
 * Somebody signed in, the way the real route's loader supplies one.
 *
 * The shell only renders for a signed in person now: the route's loader
 * resolves the identity before the element renders and redirects when there is
 * none, so a protected screen never appears for even a frame on its way to the
 * sign in screen.
 */
const IDENTITY: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Cleo Chef',
    email: 'cleo@example.test',
    role: 'chef',
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

/**
 * Renders the shell at a path and waits for its loader to settle.
 *
 * Asynchronous now, and it has to be. A route with a loader renders nothing on
 * its first pass and the element only appears once the loader resolves, so a
 * synchronous assertion straight after `render` would be looking at an empty
 * document rather than at the shell.
 */
async function renderAt(path: string) {
  // Both halves of what the real route does, because the shell reads the
  // identity from the query and falls back to this route's loader data. The
  // `id` is what `useRouteLoaderData('root')` looks itself up by, and the
  // primed entry is what the real loader's `ensureQueryData` leaves behind, so
  // nothing here fetches.
  const queryClient = new QueryClient()
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

  const router = createMemoryRouter(
    [
      {
        id: 'root',
        path: '/',
        element: <RootLayout />,
        loader: () => IDENTITY,
        children: [
          { index: true, element: <h1>Home</h1> },
          { path: 'admin', element: <h1>Admin</h1> },
          { path: 'kitchen', element: <h1>Kitchen</h1> },
          { path: '*', element: <h1>Not found</h1> },
        ],
      },
    ],
    { initialEntries: [path] },
  )

  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  // One turn of the microtask queue is all the loader needs; it returns a
  // value rather than fetching anything.
  await act(async () => {
    await Promise.resolve()
  })

  return { ...view, router, queryClient }
}

afterEach(async () => {
  delete document.documentElement.dataset['surface']
  await act(async () => {
    await changeLanguage(FALLBACK_LANGUAGE, DEFAULT_SURFACE)
  })
})

describe('RootLayout', () => {
  it('puts the density on the document element, not on a wrapper', async () => {
    await renderAt('/kitchen')
    // On the document, because dialogs and toasts render into a portal on
    // document.body, outside the React tree. A wrapper would leave every
    // overlay at admin density on a kitchen wall.
    expect(document.documentElement.dataset['surface']).toBe('kitchen')
  })

  it('falls back to the admin density for the index and for anything unmatched', async () => {
    await renderAt('/')
    expect(document.documentElement.dataset['surface']).toBe('admin')

    await renderAt('/something-that-does-not-exist')
    expect(document.documentElement.dataset['surface']).toBe('admin')
  })

  it('clears the kitchen density when you walk back out of the kitchen', async () => {
    const { router } = await renderAt('/kitchen')
    expect(document.documentElement.dataset['surface']).toBe('kitchen')

    await act(async () => {
      await router.navigate('/')
    })

    // The regression this guards: an attribute set on mount by each surface's
    // own shell would outlive the shell that set it, leaving the whole document
    // stuck at kitchen density with nothing left to clear it.
    expect(document.documentElement.dataset['surface']).toBe('admin')
  })

  it('follows the identity as it is now, not as the loader left it', async () => {
    const { queryClient } = await renderAt('/')
    expect(document.documentElement.lang).toBe('en')

    // What `PATCH /api/me` does when somebody changes their own language: it
    // writes the fresh bundle into this entry and nothing else.
    const withHindi: Identity = {
      ...IDENTITY,
      staff: { ...IDENTITY.staff, language: 'hi' },
    }

    act(() => {
      queryClient.setQueryData<Identity | null>(IDENTITY_KEY, withHindi)
    })

    // The regression this guards: the shell used to read the route loader's
    // answer, which React Router takes once and never revisits. So the write
    // above reached the cache and no screen, and changing your own language did
    // nothing at all until somebody reloaded the page.
    await waitFor(() => {
      expect(document.documentElement.lang).toBe('hi')
    })
  }) // covers: AC-16

  it('is accessible as a whole page, landmarks and headings included', async () => {
    // Built the same way as every other case here, `id` and primed entry
    // included, so this checks the shell the app really renders rather than
    // whatever a route missing half its wiring falls back to.
    const queryClient = new QueryClient()
    queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)

    const router = createMemoryRouter(
      [
        {
          id: 'root',
          path: '/',
          element: <RootLayout />,
          loader: () => IDENTITY,
          children: [{ index: true, element: <h1>Home</h1> }],
        },
      ],
      { initialEntries: ['/'] },
    )

    await expectAccessible(
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
      { page: true },
    )
  })
})
