import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { RestaurantText } from './restaurant-text'

/**
 * The case this component exists for: restaurant typed text in one language,
 * inside a page declared as another.
 */
async function withRestaurantLanguage<T>(language: string, body: () => T): Promise<T> {
  const module = await import('@/shared/session/restaurant-settings')
  const spy = vi.spyOn(module, 'restaurantLanguage').mockReturnValue(language)

  try {
    return body()
  } finally {
    spy.mockRestore()
  }
}

describe('RestaurantText', () => {
  it('marks the text as the restaurant’s language, not the reader’s', async () => {
    await withRestaurantLanguage('hi', () => {
      // The page is English. The dish is not.
      render(
        <div lang="en">
          <RestaurantText>मटर पनीर</RestaurantText>
        </div>,
      )
    })

    const dish = screen.getByText('मटर पनीर')
    expect(dish).toHaveAttribute('lang', 'hi')
  }) // covers: AC-11

  it('lets the browser work the direction out from the text itself', async () => {
    await withRestaurantLanguage('hi', () => {
      render(<RestaurantText>मटर पनीर</RestaurantText>)
    })

    // Not `ltr`, and not the restaurant's direction either. No setting on the
    // restaurant can know which of its own fields somebody typed in which
    // script, so the text is the only thing that can answer.
    expect(screen.getByText('मटर पनीर')).toHaveAttribute('dir', 'auto')
  }) // covers: AC-11

  it('renders what was typed, untouched', async () => {
    // A name with punctuation, casing, and a double space somebody chose
    // deliberately. Asserted against the DOM rather than through `getByText`,
    // which collapses runs of whitespace before it compares and so would pass
    // on a component that had quietly tidied the name up.
    const typed = 'Chef’s  SPECIAL — "the one with the chillies"'

    const { container } = await withRestaurantLanguage('en', () =>
      render(<RestaurantText>{typed}</RestaurantText>),
    )

    expect(container.textContent).toBe(typed)
  }) // covers: AC-11

  it('renders as whatever element the layout needs', async () => {
    await withRestaurantLanguage('en', () => {
      render(<RestaurantText as="p">Grilled sea bass</RestaurantText>)
    })

    expect(screen.getByText('Grilled sea bass').tagName).toBe('P')
  }) // covers: AC-11

  it('is accessible with a script that is not the page’s', async () => {
    await withRestaurantLanguage('hi', async () => {
      await expectAccessible(
        <div lang="en">
          <h1>Menu</h1>
          <RestaurantText as="p">मटर पनीर</RestaurantText>
        </div>,
      )
    })
  }) // covers: AC-11
})
