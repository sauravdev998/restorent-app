import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { KitchenTicket } from '@/kitchen/api/tickets'

import { TicketCard } from './ticket-card'

/**
 * Two things about a ticket that a passing screen test never notices, because
 * both are about the shape of what is drawn rather than about what it says.
 *
 * Both were found by driving the real pass, not by reading it.
 */

function ticket(overrides: Partial<KitchenTicket> = {}): KitchenTicket {
  return {
    id: 'ticket-1',
    sequenceNo: 3,
    tableLabel: '7',
    sentAt: new Date().toISOString(),
    status: 'queued',
    lines: [
      { id: 'line-1', dishName: 'Dal makhani', quantity: 1, status: 'queued' },
      {
        id: 'line-2',
        dishName: 'Tomato soup',
        quantity: 2,
        status: 'voided',
        voidReasonCode: 'kitchen_unavailable',
      },
    ],
    ...overrides,
  }
}

function draw(isNew: boolean) {
  return render(
    <TicketCard
      ticket={ticket()}
      since={new Date().toISOString()}
      warningAfterSeconds={600}
      lateAfterSeconds={900}
      isNew={isNew}
      freshCancelIds={new Set()}
      pending={null}
      clearing={false}
      onReady={vi.fn()}
      onUnready={vi.fn()}
      onRanOut={vi.fn()}
      onAllDone={vi.fn()}
    />,
  )
}

describe('a kitchen ticket', () => {
  it('names its round in its own element, even while it is marked new', () => {
    // The New mark and the round label sat in one element, so no element on the
    // card read "Round 3" and nothing but nothing could find the round by name
    // for as long as the mark was up, which is exactly when a chef, a screen
    // reader, and the browser scenario all look.
    //
    // Asserted over whole `textContent`, which is what a browser matches on.
    // Testing Library reads a node's direct text and skips what is nested in
    // it, so `getByText` is happy either way and proves nothing here.
    const { container } = draw(true)

    expect(screen.getByText('New')).toBeInTheDocument()

    const ownWords = Array.from(container.querySelectorAll('*')).some(
      (element) => element.textContent === 'Round 3',
    )
    expect(ownWords).toBe(true)
  })

  it('gives every dish name a type size, so it grows with the kitchen', () => {
    // Density rides on Tailwind's `text-*` utilities: the surface redefines
    // what `--text-base` means and every utility follows. A name with no size
    // class rides nothing and renders at the browser's default 16 pixels, which
    // on the pass made the one word a chef reads across the room the smallest
    // thing on the card.
    draw(false)

    for (const name of ['1 × Dal makhani', '2 × Tomato soup']) {
      const dish = screen.getByText(
        (_, element) => element?.tagName === 'P' && element.textContent === name,
      )
      expect(dish.className).toMatch(/\btext-(xs|sm|base|lg|xl|2xl|3xl)\b/)
    }
  })
})
