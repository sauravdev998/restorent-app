import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { announce } from './announce'
import { StreamWarning } from './stream-warning'

/**
 * Watched rather than replaced, so the real announcement path still runs.
 *
 * Asserting on the mounted region instead would be asserting on a timing
 * detail: the region deliberately clears itself and sets the text on the next
 * animation frame, because setting a region to the text it already holds is not
 * a change and a screen reader announces changes. Counting the calls is what
 * this component is actually responsible for.
 */
vi.mock('./announce', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./announce')>()
  return { ...actual, announce: vi.fn(actual.announce) }
})

beforeEach(() => {
  vi.clearAllMocks()
})

/**
 * The band that says live updates have stopped.
 *
 * The chip in the header says the same thing in one word, and that is the right
 * size for it while everything is fine. It is the wrong size for the case that
 * matters: a kitchen screen that has quietly stopped receiving tickets looks
 * exactly like a kitchen with no orders, and a chef reading it from three
 * metres away will not notice a small grey word in a corner.
 *
 * Two rules here are load bearing:
 *
 * **Nothing is disabled while the stream is down.** Every action on both screens
 * is an ordinary request and has nothing to do with the stream. What is lost is
 * being told about somebody else's change, and the refetch on reconnect catches
 * that up. A screen that greyed its buttons out would turn a degraded shift into
 * a stopped one.
 *
 * **Announced politely, once per drop.** Assertive would cut across whatever a
 * screen reader is in the middle of, and a connection that has been down for ten
 * minutes is not more urgent on the tenth minute than on the first.
 */
describe('StreamWarning', () => {
  it('stays out of the way while the stream is open', () => {
    render(<StreamWarning status="open" />)

    expect(screen.queryByTestId('stream-warning')).not.toBeInTheDocument()
  }) // covers: AC-16

  it('stays out of the way while the browser is still reconnecting', () => {
    // A blip must not flash a band across a kitchen screen. The hook waits out
    // a short grace period before it calls a retrying stream closed, and until
    // it does there is nothing worth interrupting anybody about.
    render(<StreamWarning status="connecting" />)

    expect(screen.queryByTestId('stream-warning')).not.toBeInTheDocument()
  }) // covers: AC-16

  it('says live updates have stopped, and what that does and does not mean', () => {
    render(<StreamWarning status="closed" />)

    expect(screen.getByTestId('stream-warning')).toBeInTheDocument()
    expect(screen.getByText('Live updates have stopped')).toBeInTheDocument()

    // The body is the half that keeps a shift running: nothing is lost, and the
    // screen catches itself up.
    expect(
      screen.getByText(
        'Everything still works and nothing is lost. Reload when you can, and the screen will catch up.',
      ),
    ).toBeInTheDocument()
  }) // covers: AC-16

  it('carries a status role, so a screen reader is told without being interrupted', () => {
    render(<StreamWarning status="closed" />)

    expect(screen.getByTestId('stream-warning')).toHaveAttribute('role', 'status')
  }) // covers: AC-16

  it('announces the drop politely, and exactly once', () => {
    const { rerender } = render(<StreamWarning status="open" />)
    expect(announce).not.toHaveBeenCalled()

    rerender(<StreamWarning status="closed" />)

    expect(announce).toHaveBeenCalledTimes(1)
    expect(announce).toHaveBeenCalledWith(
      expect.stringContaining('Live updates have stopped'),
      'polite',
    )

    // Still down is not news the second time. A stream that has been out for
    // ten minutes must not be announced on every render it survives.
    rerender(<StreamWarning status="closed" />)
    expect(announce).toHaveBeenCalledTimes(1)
  }) // covers: AC-16

  it('announces again after a recovery, so the next drop is not silent', () => {
    // The guard resets on recovery. Without it, one announcement per page load
    // would be the only one anybody ever got, and a screen that dropped twice
    // in a shift would say nothing the second time.
    const { rerender } = render(<StreamWarning status="open" />)

    rerender(<StreamWarning status="closed" />)
    expect(announce).toHaveBeenCalledTimes(1)

    rerender(<StreamWarning status="open" />)
    rerender(<StreamWarning status="closed" />)

    expect(announce).toHaveBeenCalledTimes(2)
  }) // covers: AC-16

  it('does not announce a stream that is merely reconnecting', () => {
    // The announcement belongs to the same moment the band appears. Speaking
    // during the grace period would make a one second blip audible.
    const { rerender } = render(<StreamWarning status="open" />)
    rerender(<StreamWarning status="connecting" />)

    expect(announce).not.toHaveBeenCalled()
  }) // covers: AC-16

  it('is accessible in both appearances and at every density', async () => {
    await expectAccessible(<StreamWarning status="closed" />)
  }) // covers: AC-16, AC-19
})
