import { render, screen, waitFor } from '@testing-library/react'
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

    const toasts = await screen.findByRole('list', { name: 'Notifications' })
    expect(toasts).toHaveTextContent('Round sent to the kitchen')

    const polite = document.querySelector('[aria-live="polite"]')
    await waitFor(() => {
      expect(polite).toHaveTextContent('Round sent to the kitchen')
    })

    // A real button, so Tab reaches it and Enter works. Focus was never taken.
    await user.click(screen.getByRole('button', { name: 'Dismiss' }))
    // The viewport disappears with its last toast. The live region keeps the
    // announcement, which is why this asserts on the list rather than the text.
    expect(screen.queryByRole('list', { name: 'Notifications' })).toBeNull()
  })
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
})

describe('ElapsedTime inside a table', () => {
  it('is accessible', async () => {
    await expectAccessible(<ElapsedTime since={new Date().toISOString()} />)
  })
})
