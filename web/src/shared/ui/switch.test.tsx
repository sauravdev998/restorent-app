import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { Switch } from './switch'

/**
 * The base switch, which spec 0008 added for the availability control.
 *
 * What is under test is the design system's promises on top of Radix: a real
 * switch role with its state announced, the keyboard working, and a disabled
 * switch that stays reachable rather than vanishing from the tab order.
 */
describe('Switch', () => {
  it('is a switch, named by its label, announcing whether it is on', () => {
    render(<Switch checked label="Paneer tikka available" onCheckedChange={vi.fn()} />)

    const control = screen.getByRole('switch', { name: 'Paneer tikka available' })
    expect(control).toHaveAttribute('aria-checked', 'true')
  }) // covers: AC-9 (spec 0008)

  it('asks for the opposite value when pressed, by pointer or by keyboard', async () => {
    const user = userEvent.setup()
    const onCheckedChange = vi.fn()
    render(<Switch checked label="Soup available" onCheckedChange={onCheckedChange} />)

    const control = screen.getByRole('switch', { name: 'Soup available' })
    await user.click(control)
    expect(onCheckedChange).toHaveBeenLastCalledWith(false)

    control.focus()
    await user.keyboard(' ')
    expect(onCheckedChange).toHaveBeenCalledTimes(2)
  }) // covers: AC-9 (spec 0008)

  it('stays in the tab order when disabled, and refuses to change', async () => {
    const user = userEvent.setup()
    const onCheckedChange = vi.fn()
    render(
      <Switch checked={false} disabled label="Soup available" onCheckedChange={onCheckedChange} />,
    )

    const control = screen.getByRole('switch', { name: 'Soup available' })
    expect(control).toHaveAttribute('aria-disabled', 'true')
    expect(control).not.toBeDisabled()

    await user.click(control)
    expect(onCheckedChange).not.toHaveBeenCalled()
  }) // covers: AC-9 (spec 0008)

  it('is accessible on and off, in both appearances and at every density', async () => {
    await expectAccessible(
      <>
        <Switch checked label="On switch" onCheckedChange={vi.fn()} />
        <Switch checked={false} label="Off switch" onCheckedChange={vi.fn()} />
        <Switch checked={false} disabled label="Disabled switch" onCheckedChange={vi.fn()} />
      </>,
    )
  }) // covers: AC-9 (spec 0008)
})
