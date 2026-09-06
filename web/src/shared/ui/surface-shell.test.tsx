import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { describe, expect, it } from 'vitest'

import type { Identity } from '@/shared/session/identity'
import { type Surface } from '@/shared/surface'
import { expectAccessible } from '@/test/axe'

import { SurfaceShell } from './surface-shell'

/**
 * Somebody signed in, which the shell now needs: the navigation shows the one
 * surface their role holds rather than all three.
 */
const WAITER: Identity = {
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

/**
 * The shell reaches for `NavLink`, so it only renders inside a router. A memory
 * router keeps that true without a browser history.
 */
function renderShell(
  stream: 'connecting' | 'open' | 'closed' = 'open',
  surface: Surface = 'waiter',
) {
  const router = createMemoryRouter(
    [
      {
        path: '/',
        element: (
          <SurfaceShell stream={stream} surface={surface} identity={WAITER}>
            <h1>Waiter surface</h1>
            <button type="button">Send the round</button>
          </SurfaceShell>
        ),
      },
    ],
    { initialEntries: ['/'] },
  )

  return render(<RouterProvider router={router} />)
}

describe('SurfaceShell', () => {
  it('is accessible as a whole page, landmarks and headings included', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: (
            <SurfaceShell stream="open" surface="waiter" identity={WAITER}>
              <h1>Waiter surface</h1>
            </SurfaceShell>
          ),
        },
      ],
      { initialEntries: ['/'] },
    )

    await expectAccessible(<RouterProvider router={router} />, { page: true })
  }) // covers: AC-4

  it('points the skip link at a main that can actually hold focus', () => {
    renderShell()

    const skip = screen.getByRole('link', { name: 'Skip to main content' })
    const main = screen.getByRole('main')

    // The href and the id have to agree, or the link scrolls nowhere.
    expect(skip).toHaveAttribute('href', `#${main.id}`)

    // This is the whole point of the link, and the part that silently rots.
    // Following a fragment link moves where the next Tab starts, but it only
    // moves focus itself if the target can hold focus, and a `<main>` cannot by
    // default. Without `tabIndex={-1}` the call below is a no op and the link
    // does nothing for exactly the people it exists for.
    main.focus()
    expect(main).toHaveFocus()
  }) // covers: AC-6

  it('puts the skip link first in the tab order, ahead of the navigation', async () => {
    const user = userEvent.setup()
    renderShell()

    await user.tab()

    // First Tab, before the brand and before the three surface links. A skip
    // link a keyboard user has to tab through the navigation to reach has
    // skipped nothing.
    expect(screen.getByRole('link', { name: 'Skip to main content' })).toHaveFocus()
  }) // covers: AC-6

  it('carries one of each landmark, so a screen reader has somewhere to jump to', () => {
    renderShell()

    expect(screen.getAllByRole('banner')).toHaveLength(1)
    expect(screen.getAllByRole('navigation')).toHaveLength(1)
    expect(screen.getAllByRole('main')).toHaveLength(1)
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Waiter surface')
  }) // covers: AC-6

  it('mounts both live regions once, because every announcement goes through them', () => {
    renderShell()

    // Toast and Alert share these rather than each mounting their own. Two
    // shared regions racing each other is how an announcement gets lost.
    //
    // Matched on the role rather than on `aria-live` alone, because
    // ConnectionStatus keeps a polite region of its own for the stream state,
    // and counting every polite element would fold the two together.
    expect(document.querySelectorAll('[role="status"][aria-live="polite"]')).toHaveLength(1)
    expect(document.querySelectorAll('[role="alert"][aria-live="assertive"]')).toHaveLength(1)
  }) // covers: AC-9

  it('shows the live stream state in the header', () => {
    renderShell('closed')

    expect(screen.getByRole('banner')).toHaveTextContent('Closed')
  }) // covers: AC-9

  /**
   * Where the language is a choice and where it is not, from spec 0005.
   *
   * The shell is the only place that decides this, so it is the only place it
   * can be checked. `followsRestaurantLanguage` is what it asks, and
   * `surface.test.ts` pins the answer that question gives.
   */
  it('offers the language on the shell an admin reads at a desk', () => {
    renderShell('open', 'admin')

    expect(screen.getByRole('combobox', { name: 'Language' })).toBeInTheDocument()
  }) // covers: 0005 AC-17

  it('offers the language on the shell a waiter carries', () => {
    renderShell('open', 'waiter')

    expect(screen.getByRole('combobox', { name: 'Language' })).toBeInTheDocument()
  }) // covers: 0005 AC-17

  it('offers no language on the kitchen shell', () => {
    // The shared appliance. Several chefs read it across a shift handover, so
    // it follows the restaurant rather than whoever last walked past it, and a
    // control that could change it under them does not belong there.
    renderShell('open', 'kitchen')

    expect(screen.queryByRole('combobox', { name: 'Language' })).not.toBeInTheDocument()
  }) // covers: 0005 AC-17
})
