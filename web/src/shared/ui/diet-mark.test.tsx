import { render, screen } from '@testing-library/react'
import { afterAll, describe, expect, it } from 'vitest'

import { changeLanguage } from '@/shared/i18n'
import { expectAccessible } from '@/test/axe'

import { DietMark } from './diet-mark'

/**
 * The veg, non veg, and egg mark, spec 0008 AC-13.
 *
 * The one place colour means something other than order status, so the whole
 * point of these tests is that colour is not what carries it: each mark is its
 * own shape inside the same square, and each is named in the reader's language.
 */

afterAll(async () => {
  await changeLanguage('en', 'admin')
})

describe('DietMark', () => {
  it('draws a different shape for each diet inside the same square', () => {
    render(
      <>
        <DietMark diet="veg" />
        <DietMark diet="non_veg" />
        <DietMark diet="egg" />
      </>,
    )

    const veg = screen.getByRole('img', { name: 'Vegetarian' })
    const nonVeg = screen.getByRole('img', { name: 'Non vegetarian' })
    const egg = screen.getByRole('img', { name: 'Contains egg' })

    for (const mark of [veg, nonVeg, egg]) {
      expect(mark.querySelector('rect'), 'a mark lost its square').not.toBeNull()
    }

    // Circle, triangle, oval: under forced colours and on paper, these are
    // the only difference left, so they have to be different.
    expect(veg.querySelector('circle')).not.toBeNull()
    expect(nonVeg.querySelector('path')).not.toBeNull()
    expect(egg.querySelector('ellipse')).not.toBeNull()
    expect(veg.querySelector('path, ellipse')).toBeNull()
    expect(nonVeg.querySelector('circle, ellipse')).toBeNull()
    expect(egg.querySelector('circle, path')).toBeNull()
  }) // covers: AC-13 (spec 0008)

  it('takes its colour from the diet tokens and falls back to the system text colour', () => {
    render(<DietMark diet="non_veg" />)
    const mark = screen.getByRole('img', { name: 'Non vegetarian' })

    expect(mark.getAttribute('class')).toContain('text-diet-non-veg')
    expect(mark.getAttribute('class')).toContain('forced-colors:text-[CanvasText]')
  }) // covers: AC-13 (spec 0008)

  it('names each mark in the reader’s own language', async () => {
    await changeLanguage('hi', 'admin')
    render(<DietMark diet="veg" />)

    expect(screen.getByRole('img', { name: 'शाकाहारी' })).toBeInTheDocument()
  }) // covers: AC-13 (spec 0008)

  it('is accessible in both appearances and at every density', async () => {
    await changeLanguage('en', 'admin')
    await expectAccessible(
      <>
        <DietMark diet="veg" />
        <DietMark diet="non_veg" size="md" />
        <DietMark diet="egg" />
      </>,
    )
  }) // covers: AC-13 (spec 0008)
})
