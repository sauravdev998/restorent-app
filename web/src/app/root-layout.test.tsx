import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, describe, expect, it } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { RootLayout } from './root-layout'

function renderAt(path: string) {
  const router = createMemoryRouter(
    [
      {
        path: '/',
        element: <RootLayout />,
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

  return { ...view, router }
}

afterEach(() => {
  delete document.documentElement.dataset['surface']
})

describe('RootLayout', () => {
  it('puts the density on the document element, not on a wrapper', () => {
    renderAt('/kitchen')
    // On the document, because dialogs and toasts render into a portal on
    // document.body, outside the React tree. A wrapper would leave every
    // overlay at admin density on a kitchen wall.
    expect(document.documentElement.dataset['surface']).toBe('kitchen')
  })

  it('falls back to the admin density for the index and for anything unmatched', () => {
    renderAt('/')
    expect(document.documentElement.dataset['surface']).toBe('admin')

    renderAt('/something-that-does-not-exist')
    expect(document.documentElement.dataset['surface']).toBe('admin')
  })

  it('clears the kitchen density when you walk back out of the kitchen', async () => {
    const { router } = renderAt('/kitchen')
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
      [{ path: '/', element: <RootLayout />, children: [{ index: true, element: <h1>Home</h1> }] }],
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
