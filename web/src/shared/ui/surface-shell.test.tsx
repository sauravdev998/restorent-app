import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { describe, expect, it } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { SurfaceShell } from './surface-shell'

/**
 * The shell reaches for `NavLink`, so it only renders inside a router. A memory
 * router keeps that true without a browser history.
 */
function renderShell(stream: 'connecting' | 'open' | 'closed' = 'open') {
  const router = createMemoryRouter(
    [
      {
        path: '/',
        element: (
          <SurfaceShell stream={stream} surface="waiter">
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
            <SurfaceShell stream="open" surface="waiter">
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
})
