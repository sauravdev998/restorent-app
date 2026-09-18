import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { adminFloorKey } from '@/shared/events/query-keys'
import { changeLanguage } from '@/shared/i18n'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { ToastViewport } from '@/shared/ui/toast'
import { expectAccessible } from '@/test/axe'

import { AdminFloorScreen } from './admin-floor'

/**
 * The admin's floor screen, spec 0010.
 *
 * Four promises are the subject. **The API is the authority**: every refusal
 * lands beside its box or inside the dialog, translated from its code, and a
 * range clash lists every clashing label. **A stale form never writes blind.**
 * **The occupied mark is a word and an icon**, and a busy table's removal says
 * why before it is refused. **Nothing is written into the cache ahead of the
 * server**: every save reads the floor again.
 *
 * The client is replaced wholesale, the way the other screen tests do it.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PUT: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const IDENTITY: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Ada Admin',
    email: 'ada@example.test',
    role: 'admin',
    language: null,
    mustChangePassword: false,
  },
  restaurant: {
    id: '00000000-0000-7000-8000-000000000002',
    name: 'The Test Kitchen',
    address: null,
    countryCode: 'IN',
    currencyCode: 'INR',
    currencyDecimals: 2,
    timezone: 'Asia/Kolkata',
    defaultLanguage: 'en',
    formattingLocale: 'en-IN',
  },
}

const TERRACE = '00000000-0000-7000-8000-000000000010'
const BAR = '00000000-0000-7000-8000-000000000011'
const T1 = '00000000-0000-7000-8000-000000000020'
const T2 = '00000000-0000-7000-8000-000000000021'
const LOOSE = '00000000-0000-7000-8000-000000000022'
const PATIO = '00000000-0000-7000-8000-000000000030'
const P1 = '00000000-0000-7000-8000-000000000031'
const P2 = '00000000-0000-7000-8000-000000000032'

function table(id: string, label: string, overrides: Record<string, unknown> = {}) {
  return { id, label, seats: 4, version: 1, occupied: false, ...overrides }
}

/** A loose table, a terrace of two (one taken), an empty bar, and archived rows. */
function floor(overrides: { t1?: Record<string, unknown> } = {}) {
  return {
    groups: [
      { id: null, name: null, version: null, tables: [table(LOOSE, 'Counter', { seats: null })] },
      {
        id: TERRACE,
        name: 'Terrace',
        version: 1,
        tables: [table(T1, 'T1', overrides.t1), table(T2, 'T2', { occupied: true, seats: 6 })],
      },
      { id: BAR, name: 'Bar', version: 1, tables: [] },
    ],
    archived: {
      sections: [
        {
          id: PATIO,
          name: 'Patio',
          archivedAt: '2026-09-10T10:00:00.000Z',
          tables: [
            { id: P1, label: 'P1', seats: 2 },
            { id: P2, label: 'P2', seats: null },
          ],
        },
      ],
      tables: [
        {
          id: P1,
          label: 'P1',
          seats: 2,
          sectionId: PATIO,
          sectionName: 'Patio',
          sectionLive: false,
          archivedAt: '2026-09-10T09:00:00.000Z',
        },
        {
          id: P2,
          label: 'P2',
          seats: null,
          sectionId: PATIO,
          sectionName: 'Patio',
          sectionLive: false,
          archivedAt: '2026-09-10T09:30:00.000Z',
        },
      ],
    },
  }
}

const EMPTY = {
  groups: [{ id: null, name: null, version: null, tables: [] }],
  archived: { sections: [], tables: [] },
}

let current: unknown = floor()

function answerFloor(next: unknown) {
  current = next
  vi.mocked(api.GET).mockImplementation(() => Promise.resolve({ data: current }) as never)
}

function wrap(ui: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)
  return {
    queryClient,
    ui: <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  }
}

async function mount() {
  const { queryClient, ui } = wrap(
    <>
      <AdminFloorScreen />
      <ToastViewport />
    </>,
  )
  const view = render(ui)
  await act(async () => {
    await Promise.resolve()
  })
  return { ...view, queryClient }
}

beforeEach(async () => {
  vi.clearAllMocks()
  answerFloor(floor())
  // The Hindi case below switches the interface and i18next is module wide, so
  // every test starts from English rather than from whatever ran before it.
  await changeLanguage('en', 'admin')
})

afterAll(async () => {
  await changeLanguage('en', 'admin')
})

describe('AdminFloorScreen', () => {
  it('shows loose tables first, then each section, with seats and the occupied mark', async () => {
    await mount()

    const regions = await screen.findAllByRole('region')
    expect(regions.map((region) => region.getAttribute('aria-labelledby'))).toEqual([
      'floor-group-none',
      `floor-group-${TERRACE}`,
      `floor-group-${BAR}`,
      'archived-floor-heading',
    ])

    const loose = screen.getByRole('region', { name: 'Tables with no section' })
    expect(within(loose).getByText('Counter')).toBeInTheDocument()

    const terrace = screen.getByRole('region', { name: 'Terrace' })
    const rows = within(terrace).getAllByRole('listitem')
    expect(rows.map((row) => within(row).getAllByText(/^T\d$/)[0]?.textContent)).toEqual([
      'T1',
      'T2',
    ])
    // A word, not a colour: the taken table says so, the free one says free.
    const [first, second] = rows
    expect(first && within(first).getByText('Free')).toBeInTheDocument()
    expect(second && within(second).getByText('Occupied')).toBeInTheDocument()
    expect(second && within(second).getByText('Seats 6')).toBeInTheDocument()

    expect(screen.getByText('2 sections · 3 tables · 1 occupied')).toBeInTheDocument()
    expect(
      within(screen.getByRole('region', { name: 'Bar' })).getByText(/No tables here yet/),
    ).toBeInTheDocument()
  }) // covers: AC-1, AC-14, AC-15

  it('shows skeletons while the floor loads', async () => {
    vi.mocked(api.GET).mockImplementation(() => new Promise(() => undefined) as never)
    await mount()

    expect(screen.getByRole('status', { name: '' })).toHaveTextContent('Loading')
  }) // covers: AC-1

  it('leads an empty floor to adding its first table', async () => {
    const user = userEvent.setup()
    answerFloor(EMPTY)
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add the first table' }))

    expect(screen.getByRole('dialog', { name: 'Add tables' })).toBeInTheDocument()
  }) // covers: AC-1

  it('adds a table in the chosen section and reads the floor again', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      data: {
        id: '00000000-0000-7000-8000-000000000099',
        sectionId: TERRACE,
        label: 'T3',
        seats: 2,
        version: 1,
        archivedAt: null,
      },
    })
    await mount()

    const terrace = await screen.findByRole('region', { name: 'Terrace' })
    await user.click(within(terrace).getByRole('button', { name: 'Add a table to Terrace' }))
    const form = screen.getByRole('dialog', { name: 'Add tables' })
    expect(within(form).getByRole('combobox', { name: 'Section' })).toHaveValue(TERRACE)

    await user.type(within(form).getByRole('textbox', { name: /^label/i }), 'T3')
    await user.type(within(form).getByRole('textbox', { name: /^seats/i }), '2')

    const readsBefore = vi.mocked(api.GET).mock.calls.length
    await user.click(within(form).getByRole('button', { name: 'Add table' }))

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/admin/floor/tables', {
        body: { sectionId: TERRACE, label: 'T3', seats: 2 },
      })
    })
    await waitFor(() => {
      expect(vi.mocked(api.GET).mock.calls.length).toBeGreaterThan(readsBefore)
    })
    expect(await screen.findByText('Table T3 added')).toBeInTheDocument()
  }) // covers: AC-3

  it('refuses a seat count that is not a whole number before sending anything', async () => {
    const user = userEvent.setup()
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add tables' }))
    const form = screen.getByRole('dialog', { name: 'Add tables' })
    await user.type(within(form).getByRole('textbox', { name: /^label/i }), 'T9')
    const seats = within(form).getByRole('textbox', { name: /^seats/i })
    await user.type(seats, 'four')
    await user.click(within(form).getByRole('button', { name: 'Add table' }))

    expect(seats).toHaveAccessibleDescription(expect.stringContaining('That is not a number.'))
    expect(api.POST).not.toHaveBeenCalled()
  }) // covers: AC-12

  it('puts each refused field beside its own box, in words', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: {
        error: 'invalid',
        message: 'One or more fields were not accepted.',
        fields: { label: 'already_taken', seats: 'too_large' },
      },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add tables' }))
    const form = screen.getByRole('dialog', { name: 'Add tables' })
    const label = within(form).getByRole('textbox', { name: /^label/i })
    const seats = within(form).getByRole('textbox', { name: /^seats/i })
    await user.type(label, 't1')
    await user.type(seats, '51')
    await user.click(within(form).getByRole('button', { name: 'Add table' }))

    await waitFor(() => {
      expect(label).toHaveAccessibleDescription(expect.stringContaining('That is already taken.'))
    })
    expect(seats).toHaveAccessibleDescription(
      expect.stringContaining('That is more than this can hold.'),
    )
  }) // covers: AC-12

  it('adds a numbered range, previewing it, and lists every clashing label when refused', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: {
        error: 'labels_taken',
        message: 'a live table already has one of those labels',
        labels: ['t1', 't3'],
      },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add tables' }))
    const form = screen.getByRole('dialog', { name: 'Add tables' })
    const several = within(form).getByRole('button', { name: 'Several tables' })
    await user.click(several)
    expect(several).toHaveAttribute('aria-pressed', 'true')

    await user.type(within(form).getByRole('textbox', { name: /^text before/i }), 't')
    await user.type(within(form).getByRole('textbox', { name: /^first number/i }), '1')
    await user.type(within(form).getByRole('textbox', { name: /^last number/i }), '5')
    expect(within(form).getByText('Adds 5 tables, t1 to t5.')).toBeInTheDocument()

    await user.click(within(form).getByRole('button', { name: 'Add tables' }))

    expect(
      await within(form).findByText(
        'Nothing was added. These labels are already used by other tables: t1, t3.',
      ),
    ).toHaveAttribute('role', 'alert')
    expect(api.POST).toHaveBeenCalledWith('/api/admin/floor/tables/range', {
      body: { sectionId: null, prefix: 't', from: 1, to: 5, seats: null },
    })

    vi.mocked(api.POST).mockResolvedValueOnce({
      error: { error: 'labels_taken', message: 'late', labels: [] },
    })
    await user.click(within(form).getByRole('button', { name: 'Add tables' }))
    expect(
      await within(form).findByText(
        'A label was taken a moment ago and nothing was added. Try again.',
      ),
    ).toBeInTheDocument()
  }) // covers: AC-4

  it('shows the range field refusals, including the new codes', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: {
        error: 'invalid',
        message: 'One or more fields were not accepted.',
        fields: { from: 'too_small', to: 'before_start' },
      },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add tables' }))
    const form = screen.getByRole('dialog', { name: 'Add tables' })
    await user.click(within(form).getByRole('button', { name: 'Several tables' }))
    const from = within(form).getByRole('textbox', { name: /^first number/i })
    const to = within(form).getByRole('textbox', { name: /^last number/i })
    await user.type(from, '0')
    await user.type(to, '-2')
    await user.click(within(form).getByRole('button', { name: 'Add tables' }))

    await waitFor(() => {
      expect(from).toHaveAccessibleDescription(
        expect.stringContaining('That is less than the least this allows.'),
      )
    })
    expect(to).toHaveAccessibleDescription(expect.stringContaining('That comes before the start.'))
  }) // covers: AC-12

  it('shows a stale edit the table as it now is, and saves against that', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValueOnce({
      error: { error: 'table_changed', message: 'that table changed after the form was opened' },
    } as never)
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Edit table T1' }))
    const form = screen.getByRole('dialog', { name: 'Edit table T1' })
    const seats = within(form).getByRole('textbox', { name: /^seats/i })
    expect(seats).toHaveValue('4')

    answerFloor(floor({ t1: { seats: 8, version: 3 } }))
    await user.clear(seats)
    await user.type(seats, '5')
    await user.click(within(form).getByRole('button', { name: 'Save changes' }))

    expect(
      await within(form).findByText(
        'Somebody changed this table after you opened it. The form now shows where it stands.',
      ),
    ).toBeInTheDocument()
    await waitFor(() => {
      expect(seats).toHaveValue('8')
    })

    vi.mocked(api.PUT).mockResolvedValueOnce({
      data: { id: T1, sectionId: TERRACE, label: 'T1', seats: 8, version: 4, archivedAt: null },
    } as never)
    await user.click(within(form).getByRole('button', { name: 'Save changes' }))
    await waitFor(() => {
      expect(api.PUT).toHaveBeenLastCalledWith('/api/admin/floor/tables/{id}', {
        params: { path: { id: T1 } },
        body: { sectionId: TERRACE, label: 'T1', seats: 8, version: 3 },
      })
    })
  }) // covers: AC-5, AC-13

  it('warns before removing a busy table, and says why it was refused', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'table_in_use', message: 'that table has a party at it' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Remove table T2' }))
    const dialog = screen.getByRole('dialog', { name: 'Remove table T2?' })
    expect(within(dialog).getByText(/A party is at this table now/)).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: 'Remove table' }))

    expect(
      await within(dialog).findByText(
        'A party is at that table. Close the table first, then remove it.',
      ),
    ).toHaveAttribute('role', 'alert')
    expect(api.POST).toHaveBeenCalledWith('/api/admin/floor/tables/{id}/archive', {
      params: { path: { id: T2 } },
    })
  }) // covers: AC-8

  it('says inside the dialog why a section with tables cannot be removed', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'section_not_empty', message: 'that section still has tables' },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Remove the Terrace section' }))
    const dialog = screen.getByRole('dialog', { name: 'Remove the Terrace section?' })
    expect(within(dialog).getByText(/still has 2 tables in it/)).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: 'Remove section' }))

    expect(
      await within(dialog).findByText(
        'That section still has tables in it. Move or remove them first.',
      ),
    ).toHaveAttribute('role', 'alert')
  }) // covers: AC-9

  it('restores a table into No section when its old section is removed too', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      data: { id: P2, sectionId: null, label: 'P2', seats: null, version: 3, archivedAt: null },
    })
    await mount()

    const archived = await screen.findByRole('region', { name: 'Archived' })
    expect(within(archived).getAllByText('From Patio, which is removed too')).toHaveLength(2)

    await user.click(within(archived).getByRole('button', { name: 'Put back table P2' }))
    const dialog = screen.getByRole('dialog', { name: 'Put back table P2' })
    const picker = within(dialog).getByRole('combobox', { name: 'Put it in' })
    expect(picker).toHaveValue('')
    expect(
      within(dialog).getByText(
        'Its old section, Patio, is removed too, so it starts on No section.',
      ),
    ).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: 'Put back' }))
    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/admin/floor/tables/{id}/restore', {
        params: { path: { id: P2 } },
        body: { sectionId: null },
      })
    })
  }) // covers: AC-10

  it('restores a section with its ticked tables, and names the clashing ones', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: { error: 'labels_taken', message: 'taken', labels: ['P2'] },
    })
    await mount()

    const archived = await screen.findByRole('region', { name: 'Archived' })
    expect(within(archived).getByText('2 removed tables can come back with it')).toBeInTheDocument()
    await user.click(within(archived).getByRole('button', { name: 'Put back the Patio section' }))

    const dialog = screen.getByRole('dialog', { name: 'Put back Patio' })
    const p1 = within(dialog).getByRole('checkbox', { name: /P1/ })
    const p2 = within(dialog).getByRole('checkbox', { name: /P2/ })
    expect(p1).toBeChecked()
    expect(p2).toBeChecked()

    await user.click(within(dialog).getByRole('button', { name: 'Put back' }))
    expect(await within(dialog).findByRole('alert')).toHaveTextContent(
      'These labels are already used by other tables: P2.',
    )
    expect(api.POST).toHaveBeenLastCalledWith('/api/admin/floor/sections/{id}/restore', {
      params: { path: { id: PATIO } },
      body: { tableIds: [P1, P2] },
    })

    vi.mocked(api.POST).mockResolvedValueOnce({
      data: {
        section: { id: PATIO, name: 'Patio', version: 3, archivedAt: null },
        tables: [{ id: P1, sectionId: PATIO, label: 'P1', seats: 2, version: 3, archivedAt: null }],
      },
    })
    fireEvent.click(p2)
    expect(p2).not.toBeChecked()
    await user.click(within(dialog).getByRole('button', { name: 'Put back' }))

    await waitFor(() => {
      expect(api.POST).toHaveBeenLastCalledWith('/api/admin/floor/sections/{id}/restore', {
        params: { path: { id: PATIO } },
        body: { tableIds: [P1] },
      })
    })
    expect(await screen.findByText('Patio is back with 1 table')).toBeInTheDocument()
  }) // covers: AC-11

  it('names every drag handle and announces a move, in English and in Hindi', async () => {
    await mount()

    const handle = await screen.findByRole('button', { name: 'Move table T1' })
    expect(handle).toHaveAttribute('aria-roledescription', 'movable table')
    expect(screen.getByRole('button', { name: 'Move the Terrace section' })).toHaveAttribute(
      'aria-roledescription',
      'movable section',
    )

    // The keyboard path: Space picks up, and the pick up is announced.
    handle.focus()
    fireEvent.keyDown(handle, { key: ' ', code: 'Space' })
    expect(await screen.findByText('Picked up T1. It is in position 1 of 2.')).toBeInTheDocument()
    fireEvent.keyDown(handle, { key: 'Escape', code: 'Escape' })

    await act(async () => {
      await changeLanguage('hi', 'admin')
    })
    const hindi = await screen.findByRole('button', { name: 'मेज़ T1 खिसकाएँ' })
    expect(hindi).toHaveAttribute('aria-roledescription', 'खिसकाने योग्य मेज़')
  }) // covers: AC-6, AC-19

  it('puts a refused section name beside its own box, in words', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: {
        error: 'invalid',
        message: 'One or more fields were not accepted.',
        fields: { name: 'too_long' },
      },
    })
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Add a section' }))
    const form = screen.getByRole('dialog', { name: 'Add a section' })
    const name = within(form).getByRole('textbox', { name: /^name/i })
    await user.type(name, 'A'.repeat(41))
    await user.click(within(form).getByRole('button', { name: 'Add section' }))

    await waitFor(() => {
      expect(name).toHaveAccessibleDescription(expect.stringContaining('That is too long.'))
    })

    // A name a live section already has is refused on the same box, not at the
    // top of the form: the API turns that clash into a field error on purpose.
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: {
        error: 'invalid',
        message: 'One or more fields were not accepted.',
        fields: { name: 'already_taken' },
      },
    })
    await user.clear(name)
    await user.type(name, 'terrace')
    await user.click(within(form).getByRole('button', { name: 'Add section' }))

    await waitFor(() => {
      expect(name).toHaveAccessibleDescription(expect.stringContaining('That is already taken.'))
    })
    expect(api.POST).toHaveBeenLastCalledWith('/api/admin/floor/sections', {
      body: { name: 'terrace' },
    })
  }) // covers: AC-12

  it('edits a table label, seats, and section at once, and reads the floor again', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValue({
      data: { id: T1, sectionId: null, label: 'T20', seats: 6, version: 2, archivedAt: null },
    } as never)
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Edit table T1' }))
    const form = screen.getByRole('dialog', { name: 'Edit table T1' })

    const label = within(form).getByRole('textbox', { name: /^label/i })
    await user.clear(label)
    await user.type(label, 'T20')
    const seats = within(form).getByRole('textbox', { name: /^seats/i })
    await user.clear(seats)
    await user.type(seats, '6')
    // Out of Terrace and onto the no section group, which the API puts at the
    // end of that group.
    await user.selectOptions(within(form).getByRole('combobox', { name: 'Section' }), '')

    const readsBefore = vi.mocked(api.GET).mock.calls.length
    await user.click(within(form).getByRole('button', { name: 'Save changes' }))

    await waitFor(() => {
      expect(api.PUT).toHaveBeenCalledWith('/api/admin/floor/tables/{id}', {
        params: { path: { id: T1 } },
        body: { sectionId: null, label: 'T20', seats: 6, version: 1 },
      })
    })
    await waitFor(() => {
      expect(vi.mocked(api.GET).mock.calls.length).toBeGreaterThan(readsBefore)
    })
    expect(await screen.findByText('Table T20 saved')).toBeInTheDocument()
  }) // covers: AC-5

  it('says a section chosen in an edit has been removed, and keeps the form open', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValue({
      error: { error: 'section_archived', message: 'that section is archived' },
    } as never)
    await mount()

    await user.click(await screen.findByRole('button', { name: 'Edit table T1' }))
    const form = screen.getByRole('dialog', { name: 'Edit table T1' })
    await user.selectOptions(within(form).getByRole('combobox', { name: 'Section' }), BAR)
    await user.click(within(form).getByRole('button', { name: 'Save changes' }))

    expect(
      await within(form).findByText('That section has been removed. Choose another one.'),
    ).toHaveAttribute('role', 'alert')
    // Still open, with the choice still there to correct.
    expect(screen.getByRole('dialog', { name: 'Edit table T1' })).toBeInTheDocument()
  }) // covers: AC-13

  it('puts a table back where it was, and says so, when Escape cancels a keyboard move', async () => {
    await mount()

    const handle = await screen.findByRole('button', { name: 'Move table T1' })
    handle.focus()
    fireEvent.keyDown(handle, { key: ' ', code: 'Space' })
    expect(await screen.findByText('Picked up T1. It is in position 1 of 2.')).toBeInTheDocument()

    fireEvent.keyDown(handle, { key: 'Escape', code: 'Escape' })

    expect(
      await screen.findByText('Moving cancelled. T1 is back in position 1 of 2.'),
    ).toBeInTheDocument()
    // A cancelled move is not an order, so nothing is sent.
    expect(api.PUT).not.toHaveBeenCalled()
  }) // covers: AC-6

  it('starts a restore on the old section when that section is still live', async () => {
    const user = userEvent.setup()
    const live = floor()
    answerFloor({
      ...live,
      archived: {
        sections: [],
        tables: [
          {
            id: P1,
            label: 'P1',
            seats: 2,
            sectionId: TERRACE,
            sectionName: 'Terrace',
            sectionLive: true,
            archivedAt: '2026-09-10T09:00:00.000Z',
          },
        ],
      },
    })
    await mount()

    const archived = await screen.findByRole('region', { name: 'Archived' })
    await user.click(within(archived).getByRole('button', { name: 'Put back table P1' }))

    const dialog = screen.getByRole('dialog', { name: 'Put back table P1' })
    const picker = within(dialog).getByRole('combobox', { name: 'Put it in' })
    expect(picker).toHaveValue(TERRACE)
    // The hint about a removed old section belongs to the other case only.
    expect(within(dialog).queryByText(/is removed too/)).not.toBeInTheDocument()

    vi.mocked(api.POST).mockResolvedValue({
      data: { id: P1, sectionId: TERRACE, label: 'P1', seats: 2, version: 3, archivedAt: null },
    })
    await user.click(within(dialog).getByRole('button', { name: 'Put back' }))

    await waitFor(() => {
      expect(api.POST).toHaveBeenCalledWith('/api/admin/floor/tables/{id}/restore', {
        params: { path: { id: P1 } },
        body: { sectionId: TERRACE },
      })
    })
  }) // covers: AC-10

  it('says why a table cannot come back under a label something live already has', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'name_taken', message: 'a live table already has that label' },
    })
    await mount()

    const archived = await screen.findByRole('region', { name: 'Archived' })
    await user.click(within(archived).getByRole('button', { name: 'Put back table P1' }))
    const dialog = screen.getByRole('dialog', { name: 'Put back table P1' })
    await user.click(within(dialog).getByRole('button', { name: 'Put back' }))

    expect(
      await within(dialog).findByText(
        'Something live already has that name. Rename one of them first.',
      ),
    ).toHaveAttribute('role', 'alert')
    expect(screen.getByRole('dialog', { name: 'Put back table P1' })).toBeInTheDocument()
  }) // covers: AC-10

  it('says why a section cannot come back under a name another live section has', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValue({
      error: { error: 'name_taken', message: 'a live section already has that name' },
    })
    await mount()

    const archived = await screen.findByRole('region', { name: 'Archived' })
    await user.click(within(archived).getByRole('button', { name: 'Put back the Patio section' }))
    const dialog = screen.getByRole('dialog', { name: 'Put back Patio' })
    await user.click(within(dialog).getByRole('button', { name: 'Put back' }))

    expect(
      await within(dialog).findByText(
        'Something live already has that name. Rename one of them first.',
      ),
    ).toHaveAttribute('role', 'alert')
    // Nothing came back: the dialog stays, with its tables still ticked.
    expect(within(dialog).getByRole('checkbox', { name: /P1/ })).toBeChecked()
  }) // covers: AC-11

  it('is accessible as a page, in both appearances and at every density', async () => {
    await changeLanguage('en', 'admin')
    const { queryClient, ui } = wrap(
      <main>
        <AdminFloorScreen />
      </main>,
    )
    queryClient.setQueryData(adminFloorKey, floor())

    await expectAccessible(ui)
  }) // covers: AC-19
})
