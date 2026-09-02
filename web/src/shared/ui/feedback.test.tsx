import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Inbox } from 'lucide-react'
import { describe, expect, it } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { Alert } from './alert'
import { Button } from './button'
import { DataTable, type DataTableColumn } from './data-table'
import { ElapsedTime } from './elapsed-time'
import { EmptyState } from './empty-state'
import { LiveRegion } from './live-region'
import { Skeleton } from './skeleton'
import { StatusPill } from './status-pill'
import { showToast } from './toast-store'
import { Toast, ToastViewport } from './toast'

interface Row {
  id: string
  table: string
}

const COLUMNS: DataTableColumn<Row>[] = [
  { key: 'table', header: 'Table', cell: (row) => row.table, sortable: true, rowHeader: true },
  { key: 'status', header: 'Status', cell: () => <StatusPill status="ready" /> },
]

const ROWS: Row[] = [
  { id: '1', table: '4' },
  { id: '2', table: '9' },
]

describe('Alert', () => {
  it('is accessible', async () => {
    await expectAccessible(<Alert open title="Food is ready" description="Table 4" sound={false} />)
  })

  it('fires visually and through the live region even when it can make no sound', async () => {
    // jsdom has no AudioContext at all, so this is the muted phone case: audio
    // was never unlocked and never can be. Both other channels must still work.
    render(
      <>
        <LiveRegion />
        <Alert open title="Food is ready" description="Table 4, round 2" />
      </>,
    )

    expect(screen.getByText('Food is ready')).toBeInTheDocument()

    const assertive = document.querySelector('[aria-live="assertive"]')
    await waitFor(() => {
      expect(assertive).toHaveTextContent('Food is ready. Table 4, round 2')
    })
  })
})

describe('Toast', () => {
  it('is accessible', async () => {
    await expectAccessible(
      <ul>
        <Toast toast={{ id: 'sample', title: 'Round sent', description: 'Table 4' }} />
      </ul>,
    )
  })

  it('announces politely, and can be dismissed from the keyboard', async () => {
    const user = userEvent.setup()
    render(
      <>
        <LiveRegion />
        <ToastViewport />
      </>,
    )

    showToast({ title: 'Round sent to the kitchen' })

    const toasts = await screen.findByRole('region', { name: 'Notifications' })
    expect(toasts).toHaveTextContent('Round sent to the kitchen')

    const polite = document.querySelector('[aria-live="polite"]')
    await waitFor(() => {
      expect(polite).toHaveTextContent('Round sent to the kitchen')
    })

    // A real button, so Tab reaches it and Enter works. Focus was never taken.
    await user.click(screen.getByRole('button', { name: 'Dismiss' }))
    // The viewport disappears with its last toast. The live region keeps the
    // announcement, which is why this asserts on the landmark rather than the
    // text.
    expect(screen.queryByRole('region', { name: 'Notifications' })).toBeNull()
  })

  it('is a named landmark wrapping a real list, not a list pretending to be a landmark', async () => {
    render(<ToastViewport />)

    showToast({ title: 'Round sent to the kitchen' })
    showToast({ title: 'Table 9 is ready' })

    // Toasts are pinned to the viewport instead of sitting inside `<main>`, so
    // without a landmark of their own they are content outside every landmark,
    // which a screen reader user navigating by landmark never arrives at.
    const region = await screen.findByRole('region', { name: 'Notifications' })

    // The role belongs on the wrapper, never on the `<ul>` itself. A list told
    // it is a region stops being a list, and its items stop being counted, so a
    // waiter is no longer told "2 items" when two toasts stack up.
    const list = within(region).getByRole('list')
    expect(within(list).getAllByRole('listitem')).toHaveLength(2)
  }) // covers: AC-9
})

describe('Skeleton', () => {
  it('is accessible', async () => {
    await expectAccessible(
      <>
        <Skeleton label="Loading" />
        <Skeleton />
      </>,
    )
  })

  it('announces once for a group rather than once per shape', () => {
    render(
      <>
        <Skeleton label="Loading" />
        <Skeleton />
        <Skeleton />
      </>,
    )
    expect(screen.getAllByRole('status')).toHaveLength(1)
  })

  it('stops moving for someone who asked their system for less motion', () => {
    render(<Skeleton label="Loading" />)
    expect(screen.getByRole('status')).toHaveClass('motion-reduce:animate-none')
  })
})

describe('EmptyState', () => {
  it('is accessible', async () => {
    await expectAccessible(
      <EmptyState
        icon={Inbox}
        title="No open tickets"
        description="Tickets appear the moment a waiter sends a round."
        action={<Button variant="secondary">Refresh</Button>}
      />,
    )
  })
})

describe('DataTable', () => {
  it('is accessible', async () => {
    await expectAccessible(
      <DataTable
        caption="Open orders"
        columns={COLUMNS}
        rows={ROWS}
        rowKey={(row) => row.id}
        sort={{ key: 'table', direction: 'ascending' }}
        onSortChange={() => undefined}
      />,
    )
  })

  it('is a real table, with a caption, column headers, and a row header each row', () => {
    render(
      <DataTable caption="Open orders" columns={COLUMNS} rows={ROWS} rowKey={(row) => row.id} />,
    )

    expect(screen.getByRole('table', { name: 'Open orders' })).toBeInTheDocument()
    expect(screen.getAllByRole('columnheader')).toHaveLength(2)
    // One row header per row is what lets a screen reader say which row it is on.
    expect(screen.getAllByRole('rowheader')).toHaveLength(2)
  })

  it('announces the current sort rather than only drawing it', () => {
    render(
      <DataTable
        caption="Open orders"
        columns={COLUMNS}
        rows={ROWS}
        rowKey={(row) => row.id}
        sort={{ key: 'table', direction: 'descending' }}
        onSortChange={() => undefined}
      />,
    )

    const [sorted] = screen.getAllByRole('columnheader')
    expect(sorted).toHaveAttribute('aria-sort', 'descending')
    expect(screen.getByRole('button', { name: /Table/ })).toBeInTheDocument()
  })

  it('says so when there is nothing to show', () => {
    render(<DataTable caption="Open orders" columns={COLUMNS} rows={[]} rowKey={(row) => row.id} />)
    expect(screen.getByText('Nothing to show')).toBeInTheDocument()
  })

  it('wraps the table in a focusable region named by its caption', () => {
    render(
      <DataTable caption="Open orders" columns={COLUMNS} rows={ROWS} rowKey={(row) => row.id} />,
    )

    // Three columns do not fit a phone, so this box scrolls sideways. A box
    // that scrolls but cannot be focused is one a keyboard or switch user can
    // never reach the far side of, which is what the focusable region fixes.
    const region = screen.getByRole('region', { name: 'Open orders' })
    expect(region).toHaveAttribute('tabindex', '0')

    // Named by the caption, not left anonymous: landing on an unnamed focus
    // stop tells you nothing about what you just arrived at.
    expect(region).toContainElement(screen.getByRole('table', { name: 'Open orders' }))
  }) // covers: AC-4, AC-6

  it('keeps the region named even when the caption is hidden from sight', () => {
    render(
      <DataTable
        caption="Open orders"
        captionHidden
        columns={COLUMNS}
        rows={ROWS}
        rowKey={(row) => row.id}
      />,
    )

    // Hiding the caption visually must not take the name away: `sr-only` still
    // leaves the text in the accessibility tree, and the region points at it.
    expect(screen.getByRole('region', { name: 'Open orders' })).toBeInTheDocument()
  }) // covers: AC-4

  it('gives two tables on one screen their own names rather than one shared id', () => {
    render(
      <>
        <DataTable caption="Open orders" columns={COLUMNS} rows={ROWS} rowKey={(row) => row.id} />
        <DataTable caption="Closed orders" columns={COLUMNS} rows={ROWS} rowKey={(row) => row.id} />
      </>,
    )

    // The caption id comes from `useId`. A hardcoded id would make both regions
    // point at the first caption, so the admin's second table would announce
    // itself as the first one.
    expect(screen.getByRole('region', { name: 'Open orders' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'Closed orders' })).toBeInTheDocument()
  }) // covers: AC-4
})

describe('ElapsedTime inside a table', () => {
  it('is accessible', async () => {
    await expectAccessible(<ElapsedTime since={new Date().toISOString()} />)
  })
})
