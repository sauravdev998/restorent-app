import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { DesignGallery } from './design-gallery'

/**
 * The gallery is the one screen that proves the tokens rather than using them,
 * so what is worth pinning here is its coverage: both appearances at once, all
 * three densities on demand, every status present. The individual components
 * are proved by their own axe runs, so this does not repeat them.
 */
describe('DesignGallery', () => {
  it('shows both appearances at the same time, forced by data-theme', () => {
    const { container } = render(<DesignGallery />)

    // A media query is document wide, so without the `data-theme` hook the two
    // appearances could never sit side by side and the light theme would ship
    // looked at by nobody.
    expect(container.querySelector('[data-theme="dark"]')).not.toBeNull()
    expect(container.querySelector('[data-theme="light"]')).not.toBeNull()
  }) // covers: AC-3, AC-15

  it('rescales every panel when the density changes, with one control', async () => {
    const user = userEvent.setup()
    const { container } = render(<DesignGallery />)

    const surfacesNow = () =>
      [...container.querySelectorAll('[data-surface]')].map((el) => el.getAttribute('data-surface'))

    // Admin is where it starts, both panels agreeing.
    expect(surfacesNow()).toEqual(['admin', 'admin'])

    await user.click(screen.getByRole('button', { name: 'Kitchen', pressed: false }))

    // Both panels move together. A toggle that only reached one of them would
    // make the gallery lie about what a kitchen screen looks like.
    expect(surfacesNow()).toEqual(['kitchen', 'kitchen'])
    expect(screen.getByRole('button', { name: 'Kitchen' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Admin' })).toHaveAttribute('aria-pressed', 'false')
  }) // covers: AC-2, AC-15

  it('renders every order status, so none of them ships unlooked at', () => {
    render(<DesignGallery />)

    // Four real states from the enums plus the one derived emphasis. Each
    // appears in both panels, hence the pair.
    for (const word of ['Cooking', 'Ready', 'Served', 'Voided', 'Late']) {
      expect(screen.getAllByText(word).length).toBeGreaterThanOrEqual(2)
    }
  }) // covers: AC-5, AC-15

  it('offers the overlays that cannot live inside a forced panel', () => {
    render(<DesignGallery />)

    // Dialog, alert, and toasts render into a portal on `document.body`, so
    // they follow the document rather than a panel. The gallery has to drive
    // them from outside the panels or they would never be seen at all.
    expect(screen.getByRole('button', { name: 'Open the dialog' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Raise the ready alert' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Show a toast: Ready' })).toBeInTheDocument()
  }) // covers: AC-15
})
