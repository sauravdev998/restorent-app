import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, describe, expect, it } from 'vitest'

import type { Identity } from '@/shared/session/identity'
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
  const router = createMemoryRouter(
    [
      {
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
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  // One turn of the microtask queue is all the loader needs; it returns a
  // value rather than fetching anything.
  await act(async () => {
    await Promise.resolve()
  })

  return { ...view, router }
}

afterEach(() => {
  delete document.documentElement.dataset['surface']
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

  it('is accessible as a whole page, landmarks and headings included', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <RootLayout />,
          loader: () => IDENTITY,
          children: [{ index: true, element: <h1>Home</h1> }],
        },
      ],
      { initialEntries: ['/'] },
    )

    await expectAccessible(
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
      { page: true },
    )
  })
})
