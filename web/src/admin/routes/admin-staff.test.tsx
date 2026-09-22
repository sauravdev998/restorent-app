import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { StaffMember } from '@/admin/api/staff'
import { api } from '@/shared/api/client'
import { staffKey } from '@/shared/events/query-keys'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { ToastViewport } from '@/shared/ui/toast'
import { dismissToast, subscribeToToasts } from '@/shared/ui/toast-store'
import { expectAccessible } from '@/test/axe'

import { AdminStaffScreen } from './admin-staff'

/**
 * The admin's staff screen and its dialogs, spec 0009.
 *
 * Three promises are the subject. **The password shown is the one that was
 * written**: it exists nowhere else, so the hand over panel must never drift
 * from what was sent. **The admin's own row invites nothing**: the API refuses
 * all of it, so the screen offers none of it. **A stale edit never writes
 * blind**: a refused save shows the person as they now stand.
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
    kitchenWarningAfterSeconds: 600,
    kitchenLateAfterSeconds: 900,
    version: 1,
  },
}

/** One staff member as the list carries them. */
function member(overrides: Partial<StaffMember> & Pick<StaffMember, 'id' | 'displayName'>) {
  return {
    email: `${overrides.displayName.split(' ')[0]?.toLowerCase() ?? 'x'}@example.test`,
    role: 'waiter',
    active: true,
    mustChangePassword: false,
    lastSignInAt: '2026-09-14T10:00:00.000Z',
    version: 1,
    ...overrides,
  } satisfies StaffMember
}

const ADA = member({ id: IDENTITY.staff.id, displayName: 'Ada Admin', role: 'admin' })
const BO = member({
  id: '00000000-0000-7000-8000-000000000011',
  displayName: 'Bo Waiter',
  mustChangePassword: true,
  lastSignInAt: null,
})
const CY = member({
  id: '00000000-0000-7000-8000-000000000012',
  displayName: 'Cy Chef',
  role: 'chef',
  version: 4,
})
const DI = member({
  id: '00000000-0000-7000-8000-000000000013',
  displayName: 'Di Gone',
  active: false,
})

/** What `GET /api/staff` answers, changeable mid test. */
function answerStaff(people: StaffMember[]) {
  vi.mocked(api.GET).mockImplementation(() => Promise.resolve({ data: { staff: people } }) as never)
}

/** A request that stays in flight until the test says otherwise. */
function held(method: typeof api.POST | typeof api.PUT | typeof api.PATCH) {
  const control = { settle: (_: unknown) => undefined as void }
  vi.mocked(method).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        control.settle = resolve
      }) as never,
  )
  return control
}

function screenUnderTest(queryClient: QueryClient) {
  // No loader, so the router is ready on the first render. The identity comes
  // from the cache, which is what `useIdentity` prefers anyway.
  const router = createMemoryRouter(
    [
      {
        id: 'root',
        path: '/',
        element: (
          <>
            <AdminStaffScreen />
            <ToastViewport />
          </>
        ),
      },
    ],
    { initialEntries: ['/'] },
  )

  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}

function freshClient(people?: StaffMember[]) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, IDENTITY)
  if (people) queryClient.setQueryData(staffKey, people)
  return queryClient
}

async function mount() {
  const view = render(screenUnderTest(freshClient()))
  await screen.findByText('Bo Waiter')
  return view
}

beforeEach(() => {
  vi.clearAllMocks()
  answerStaff([ADA, BO, CY, DI])
})

// Toasts live in a module store that outlives a render, so one test's
// confirmation would otherwise still be on screen in the next.
afterEach(() => {
  let showing: string[] = []
  const unsubscribe = subscribeToToasts((toasts) => {
    showing = toasts.map((toast) => toast.id)
  })
  unsubscribe()
  for (const id of showing) dismissToast(id)
})

describe('AdminStaffScreen', () => {
  it('is accessible, both lists included', async () => {
    await expectAccessible(screenUnderTest(freshClient([ADA, BO, CY, DI])))
  }) // covers: AC-18, AC-21

  it('lists the working people and the switched off ones apart, with the counts', async () => {
    await mount()

    expect(
      screen.getByText('3 people working here · 1 has not chosen a password yet · 1 switched off'),
    ).toBeInTheDocument()

    const working = screen.getByRole('region', { name: 'Working here' })
    expect(within(working).getByText('Bo Waiter')).toBeInTheDocument()
    expect(within(working).getByText('Never')).toBeInTheDocument()
    expect(within(working).getByText('Has not chosen a password yet')).toBeInTheDocument()
    expect(within(working).queryByText('Di Gone')).not.toBeInTheDocument()

    const off = screen.getByRole('region', { name: 'Switched off' })
    expect(within(off).getByText('Di Gone')).toBeInTheDocument()
    expect(within(off).getByRole('button', { name: 'Bring Di Gone back' })).toBeInTheDocument()
  }) // covers: AC-6, AC-18

  it('offers nothing on the admin’s own row, and all four actions on everyone else’s', async () => {
    await mount()

    const rows = within(screen.getByRole('region', { name: 'Working here' })).getAllByRole('row')
    const own = rows.find((row) => row.textContent.includes('Ada Admin'))
    const other = rows.find((row) => row.textContent.includes('Cy Chef'))

    expect(own).toBeDefined()
    expect(other).toBeDefined()
    if (!own || !other) return

    expect(within(own).getByText('This is you')).toBeInTheDocument()
    expect(within(own).queryAllByRole('button')).toHaveLength(0)
    expect(within(other).getAllByRole('button')).toHaveLength(4)
  }) // covers: AC-12, AC-18

  it('brings a switched off account back and reads the list again', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValueOnce({ data: { ...DI, active: true } })
    await mount()

    const readsBefore = vi.mocked(api.GET).mock.calls.length
    await user.click(screen.getByRole('button', { name: 'Bring Di Gone back' }))

    expect(api.POST).toHaveBeenCalledWith('/api/staff/{id}/reactivate', {
      params: { path: { id: DI.id } },
    })
    expect(await screen.findByText('Di Gone can sign in again.')).toBeInTheDocument()
    await waitFor(() => {
      expect(vi.mocked(api.GET).mock.calls.length).toBeGreaterThan(readsBefore)
    })
  }) // covers: AC-11

  it('says inside the dialog why switching somebody off was refused', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: { error: 'staff_inactive', message: 'that account is switched off' },
    })
    await mount()

    await user.click(screen.getByRole('button', { name: "Switch Cy Chef's account off" }))
    const dialog = screen.getByRole('dialog', { name: "Switch Cy Chef's account off?" })
    await user.click(within(dialog).getByRole('button', { name: 'Switch it off' }))

    expect(
      await within(dialog).findByText('That account is switched off. Bring it back first.'),
    ).toHaveAttribute('role', 'alert')
  }) // covers: AC-10, AC-13
})

describe('CreateStaffDialog', () => {
  it('hands over the password that was sent, even if the box changed while it saved', async () => {
    // The regression: the panel read the live field rather than what was
    // submitted, and a pending mutation takes the callbacks of the latest
    // render. An edit made while the request was in flight was what the admin
    // read out, and it was not what the server hashed.
    const user = userEvent.setup()
    const request = held(api.POST)
    await mount()

    await user.click(screen.getByRole('button', { name: 'Add somebody' }))
    const dialog = screen.getByRole('dialog', { name: 'Add somebody' })
    await user.type(within(dialog).getByRole('textbox', { name: /^Name/ }), 'Eve Waiter')
    await user.type(within(dialog).getByRole('textbox', { name: /^Email address/ }), 'eve@x.test')
    const password = within(dialog).getByLabelText(/^First password/)
    await user.type(password, 'the-one-sent-1')
    await user.click(within(dialog).getByRole('button', { name: 'Create the account' }))

    expect(api.POST).toHaveBeenCalledWith('/api/staff', {
      body: {
        displayName: 'Eve Waiter',
        email: 'eve@x.test',
        password: 'the-one-sent-1',
        role: 'waiter',
      },
    })

    await user.clear(password)
    await user.type(password, 'typed-afterwards-2')

    const eve = member({ id: '00000000-0000-7000-8000-000000000014', displayName: 'Eve Waiter' })
    await act(async () => {
      request.settle({ data: { ...eve, email: 'eve@x.test', mustChangePassword: true } })
      await Promise.resolve()
    })

    const done = await screen.findByRole('dialog', { name: 'Eve Waiter can sign in now' })
    expect(within(done).getByText('the-one-sent-1')).toBeInTheDocument()
    expect(within(done).queryByText('typed-afterwards-2')).not.toBeInTheDocument()
  }) // covers: AC-1

  it('puts a taken address beside the address box', async () => {
    const user = userEvent.setup()
    vi.mocked(api.POST).mockResolvedValueOnce({
      error: { error: 'invalid', message: 'no', fields: { email: 'already_taken' } },
    })
    await mount()

    await user.click(screen.getByRole('button', { name: 'Add somebody' }))
    const dialog = screen.getByRole('dialog', { name: 'Add somebody' })
    await user.type(within(dialog).getByRole('textbox', { name: /^Name/ }), 'Bo Again')
    await user.type(within(dialog).getByRole('textbox', { name: /^Email address/ }), 'bo@x.test')
    await user.type(within(dialog).getByLabelText(/^First password/), 'a-long-password')
    await user.click(within(dialog).getByRole('button', { name: 'Create the account' }))

    const email = within(dialog).getByRole('textbox', { name: /^Email address/ })
    await waitFor(() => {
      expect(email).toHaveAttribute('aria-invalid', 'true')
    })
    expect(within(dialog).getByRole('textbox', { name: /^Name/ })).not.toHaveAttribute(
      'aria-invalid',
    )
    // Still the form, not the hand over panel.
    expect(screen.queryByRole('dialog', { name: /can sign in now/ })).not.toBeInTheDocument()
  }) // covers: AC-2
})

describe('ResetPasswordDialog', () => {
  it('hands over the password that was written, even if the box changed while it saved', async () => {
    // The same regression as the create dialog, where the password also went
    // up through the closure rather than as the mutation's variable.
    const user = userEvent.setup()
    const request = held(api.POST)
    await mount()

    await user.click(screen.getByRole('button', { name: "Reset Bo Waiter's password" }))
    const dialog = screen.getByRole('dialog', { name: "Reset Bo Waiter's password" })
    const password = within(dialog).getByLabelText(/^First password/)
    await user.type(password, 'the-one-sent-1')
    await user.click(within(dialog).getByRole('button', { name: 'Reset it' }))

    expect(api.POST).toHaveBeenCalledWith('/api/staff/{id}/password', {
      params: { path: { id: BO.id } },
      body: { password: 'the-one-sent-1' },
    })

    await user.clear(password)
    await user.type(password, 'typed-afterwards-2')

    await act(async () => {
      request.settle({ response: new Response(null, { status: 204 }) })
      await Promise.resolve()
    })

    const done = await screen.findByRole('dialog', { name: 'Bo Waiter has a new password' })
    expect(within(done).getByText('the-one-sent-1')).toBeInTheDocument()
    expect(within(done).queryByText('typed-afterwards-2')).not.toBeInTheDocument()
  }) // covers: AC-9
})

describe('EditStaffDialog', () => {
  it('says a role change to the role somebody already holds changed nothing', async () => {
    // The API accepts it and writes nothing, revoking no session, so "signed
    // out everywhere" would be telling the admin something that did not happen.
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValueOnce({ data: CY } as never)
    await mount()

    await user.click(screen.getByRole('button', { name: 'Change what Cy Chef may do' }))
    const dialog = screen.getByRole('dialog', { name: 'What may Cy Chef do?' })
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))

    expect(
      await screen.findByText(
        'Cy Chef is already a Chef. Nothing changed, and they are still signed in.',
      ),
    ).toBeInTheDocument()
    expect(screen.queryByText(/has been signed out everywhere/)).not.toBeInTheDocument()
  }) // covers: AC-8

  it('says a real role change signed that person out', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PUT).mockResolvedValueOnce({
      data: { ...CY, role: 'waiter', version: 5 },
    } as never)
    await mount()

    await user.click(screen.getByRole('button', { name: 'Change what Cy Chef may do' }))
    const dialog = screen.getByRole('dialog', { name: 'What may Cy Chef do?' })
    await user.selectOptions(within(dialog).getByRole('combobox', { name: /^Role/ }), 'waiter')
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))

    expect(api.PUT).toHaveBeenCalledWith('/api/staff/{id}/role', {
      params: { path: { id: CY.id } },
      body: { role: 'waiter', version: 4 },
    })
    expect(
      await screen.findByText('Cy Chef is now a Waiter, and has been signed out everywhere.'),
    ).toBeInTheDocument()
  }) // covers: AC-8

  it('shows a stale rename the person as they now stand, and saves against that', async () => {
    const user = userEvent.setup()
    vi.mocked(api.PATCH).mockResolvedValueOnce({
      error: { error: 'staff_changed', message: 'that person changed' },
    } as never)
    await mount()

    await user.click(screen.getByRole('button', { name: 'Rename Cy Chef' }))
    const dialog = screen.getByRole('dialog', { name: 'Rename Cy Chef' })
    const name = within(dialog).getByRole('textbox', { name: /^Name/ })

    // Another admin renamed them meanwhile.
    answerStaff([ADA, BO, { ...CY, displayName: 'Cy Cook', version: 5 }, DI])
    await user.clear(name)
    await user.type(name, 'Cyrus Chef')
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))

    expect(
      await within(dialog).findByText(
        'Somebody changed this person first. Their details above are how they stand now.',
      ),
    ).toBeInTheDocument()
    await waitFor(() => {
      expect(name).toHaveValue('Cy Cook')
    })

    vi.mocked(api.PATCH).mockResolvedValueOnce({
      data: { ...CY, displayName: 'Cy Cook', version: 6 },
    } as never)
    await user.click(within(dialog).getByRole('button', { name: 'Save' }))

    await waitFor(() => {
      expect(api.PATCH).toHaveBeenLastCalledWith('/api/staff/{id}', {
        params: { path: { id: CY.id } },
        body: { displayName: 'Cy Cook', version: 5 },
      })
    })
  }) // covers: AC-14
})
