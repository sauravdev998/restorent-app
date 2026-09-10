import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { ConnectionStatus } from './connection-status'

/**
 * The one word in the header that says whether the stream is up.
 *
 * It is deliberately small. The band does the shouting when it matters; this
 * says where things stand the rest of the time, and it must say it in words
 * rather than in a colour alone, because a colour is the one signal a person
 * with low vision or forced colours on may not get.
 */
describe('ConnectionStatus', () => {
  it('names each state in words, not colour alone', () => {
    const { rerender } = render(<ConnectionStatus status="connecting" />)
    expect(screen.getByTestId('stream-status')).toHaveTextContent('Connecting')

    rerender(<ConnectionStatus status="open" />)
    expect(screen.getByTestId('stream-status')).toHaveTextContent('Connected')

    rerender(<ConnectionStatus status="closed" />)
    expect(screen.getByTestId('stream-status')).toHaveTextContent('Closed')
  }) // covers: AC-16

  it('is a polite live region, so a drop is mentioned rather than shouted', () => {
    render(<ConnectionStatus status="closed" />)

    expect(screen.getByTestId('stream-status')).toHaveAttribute('aria-live', 'polite')
  }) // covers: AC-16

  it('reads as down only when live updates have actually stopped', () => {
    // `closed` is the state the screens react to. It covers both a browser that
    // has given up for good and one that has been retrying long enough that the
    // difference has stopped mattering to whoever is reading the screen.
    const { rerender } = render(<ConnectionStatus status="open" />)
    expect(screen.getByTestId('stream-status').className).not.toMatch(/text-status-late/)

    rerender(<ConnectionStatus status="connecting" />)
    expect(screen.getByTestId('stream-status').className).not.toMatch(/text-status-late/)

    rerender(<ConnectionStatus status="closed" />)
    expect(screen.getByTestId('stream-status').className).toMatch(/text-status-late/)
  }) // covers: AC-16

  it('is accessible in every state, both appearances, every density', async () => {
    await expectAccessible(<ConnectionStatus status="closed" />)
    await expectAccessible(<ConnectionStatus status="connecting" />)
    await expectAccessible(<ConnectionStatus status="open" />)
  }) // covers: AC-19
})
