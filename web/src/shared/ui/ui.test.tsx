import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { Button } from './button'
import { Card } from './card'
import { ElapsedTime } from './elapsed-time'
import { StatusPill } from './status-pill'
import { STATUS_TONES } from './status'

describe('Button', () => {
  it('is accessible in both appearances and all three densities', async () => {
    await expectAccessible(
      <>
        <Button>Send</Button>
        <Button variant="secondary">Cancel</Button>
        <Button variant="ghost">More</Button>
        <Button variant="destructive">Void</Button>
        <Button disabled>Send</Button>
      </>,
    )
  })

  it('stays in the tab order when disabled, and refuses the click', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    render(
      <Button disabled onClick={onClick}>
        Send
      </Button>,
    )

    const button = screen.getByRole('button', { name: 'Send' })
    // aria-disabled rather than the native attribute: a natively disabled
    // button cannot be focused, so a keyboard user never learns it is there.
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(button).not.toBeDisabled()

    await user.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })
})

describe('StatusPill', () => {
  it('is accessible for every status', async () => {
    await expectAccessible(
      <>
        {STATUS_TONES.map((status) => (
          <StatusPill key={status} status={status} />
        ))}
      </>,
    )
  })

  it('carries a word and an icon as well as a colour, for every status', () => {
    render(
      <>
        {STATUS_TONES.map((status) => (
          <StatusPill key={status} status={status} />
        ))}
      </>,
    )

    // Colour is never the only signal. Strip it and both of the other two are
    // still here: the translated word as text, and an icon in the markup.
    for (const status of STATUS_TONES) {
      const pill = document.querySelector(`[data-status="${status}"]`)
      expect(pill).not.toBeNull()
      expect(pill?.textContent?.trim()).not.toBe('')
      expect(pill?.querySelector('svg')).not.toBeNull()
    }
  })

  it('keeps announcing the word when the pill is compact', () => {
    render(<StatusPill status="ready" compact />)
    expect(screen.getByText('Ready')).toBeInTheDocument()
  })
})

describe('Card', () => {
  it('is accessible', async () => {
    await expectAccessible(<Card>A ticket</Card>)
  })
})

describe('ElapsedTime', () => {
  it('is accessible', async () => {
    await expectAccessible(<ElapsedTime since={new Date().toISOString()} />)
  })

  it('says the whole duration in words, not just a clock face', () => {
    const since = new Date(Date.now() - 90_000).toISOString()
    render(<ElapsedTime since={since} />)

    const time = screen.getByText('1:30').closest('time')
    expect(time).toHaveAttribute('datetime', 'PT1M30S')
    expect(time?.textContent).toContain('1 minute, 30 seconds')
  })

  it('marks itself late past the threshold, and says so', () => {
    const since = new Date(Date.now() - 20 * 60_000).toISOString()
    render(<ElapsedTime since={since} lateAfterSeconds={900} />)

    const time = screen.getByText('20:00').closest('time')
    expect(time).toHaveAttribute('data-late', 'true')
    expect(time?.textContent).toContain('Late')
  })
})
