import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { IDENTITY_KEY, type Identity } from '@/shared/session/identity'
import { SIGN_IN_PATH } from '@/shared/session/signed-out'
import { expectAccessible } from '@/test/axe'

import { Account } from './account'

/**
 * Somebody's own account: their name, their language, their password, the way
 * out.
 *
 * The client is replaced wholesale for the same reason the auth screens replace
 * it: the real one builds a `Request` from a relative address, which jsdom
 * refuses outright. What is under test here is what the screen does with an
 * answer, which this hands it directly.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

const IDENTITY: Identity = {
  staff: {
    id: '00000000-0000-7000-8000-000000000001',
    displayName: 'Ada Owner',
    email: 'ada@example.test',
    role: 'admin',
    language: null,
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

/**
 * Mounts the screen the way the real tree does.
 *
 * Both halves, because the screen reads its identity through `useIdentity`,
 * which prefers the query and falls back to the route named `root`. A mount
 * missing either one would be testing a screen the app never renders.
 */
async function mount(identity: Identity = IDENTITY) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData<Identity | null>(IDENTITY_KEY, identity)

  const router = createMemoryRouter(
    [
      { id: 'root', path: '/', element: <Account />, loader: () => identity },
      { path: SIGN_IN_PATH, element: <p>the sign in screen</p> },
    ],
    { initialEntries: ['/'] },
  )

  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  await act(async () => {
    await Promise.resolve()
  })

  return { ...view, router, queryClient }
}

/** Answers the next call the way the API answers a refused one. */
function refuseWith(
  method: 'POST' | 'PATCH',
  status: number,
  body: { error: string; message: string; fields?: Record<string, string> },
) {
  vi.mocked(api[method]).mockResolvedValue({
    error: body,
    response: new Response(null, { status }),
  })
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('Account', () => {
  it('is accessible, headings and labelled controls included', async () => {
    await expectAccessible(
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider
          router={createMemoryRouter(
            [{ id: 'root', path: '/', element: <Account />, loader: () => IDENTITY }],
            { initialEntries: ['/'] },
          )}
        />
      </QueryClientProvider>,
    )
  }) // covers: AC-21

  it('offers following the restaurant as a real choice, not an absence', async () => {
    // Nobody's language is "unset". Somebody who has not picked one is asking
    // to read whatever their restaurant reads, and that is a choice they can
    // make again after picking Hindi, so it has to be in the list.
    await mount()

    const language = screen.getByRole('combobox', { name: /Your language/ })

    expect(language).toHaveValue('')
    expect(screen.getByRole('option', { name: 'Whatever the restaurant uses' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: 'हिन्दी' })).toBeInTheDocument()
  }) // covers: AC-16

  it('shows the person their own address and their own stored language', async () => {
    await mount({ ...IDENTITY, staff: { ...IDENTITY.staff, language: 'hi' } })

    expect(screen.getByText('ada@example.test')).toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: /Your language/ })).toHaveValue('hi')
  }) // covers: AC-16

  it('says the current password was wrong beside the current password box', async () => {
    // The one field error that matters here, and the one place it can go. Put
    // on the new password box it would send somebody off inventing a different
    // new password, which was never the problem.
    refuseWith('POST', 400, {
      error: 'invalid',
      message: 'no',
      fields: { currentPassword: 'incorrect' },
    })

    await mount()

    await userEvent.type(screen.getByLabelText(/Current password/), 'not-the-right-one')
    await userEvent.type(screen.getByLabelText(/New password/), 'a-long-enough-password')
    await userEvent.click(screen.getByRole('button', { name: 'Change password' }))

    const current = screen.getByLabelText(/Current password/)
    await waitFor(() => {
      expect(current).toHaveAttribute('aria-invalid', 'true')
    })
    expect(await screen.findByText('That is not right.')).toBeInTheDocument()

    // And only that box. Marking the new one too would say two things failed.
    expect(screen.getByLabelText(/New password/)).not.toHaveAttribute('aria-invalid')
  }) // covers: AC-14

  it('keeps what was typed in the password boxes when the change is refused', async () => {
    // Clearing the form on a refusal makes somebody who mistyped one character
    // type the whole thing again, and the boxes are the two they cannot see.
    refuseWith('POST', 400, {
      error: 'invalid',
      message: 'no',
      fields: { currentPassword: 'incorrect' },
    })

    await mount()

    await userEvent.type(screen.getByLabelText(/Current password/), 'not-the-right-one')
    await userEvent.type(screen.getByLabelText(/New password/), 'a-long-enough-password')
    await userEvent.click(screen.getByRole('button', { name: 'Change password' }))

    await waitFor(() => {
      expect(screen.getByLabelText(/Current password/)).toHaveAttribute('aria-invalid', 'true')
    })
    expect(screen.getByLabelText(/Current password/)).toHaveValue('not-the-right-one')
    expect(screen.getByLabelText(/New password/)).toHaveValue('a-long-enough-password')
  }) // covers: AC-14

  it('clears both password boxes once the change goes through', async () => {
    // The other half. A password that was accepted is one nobody should find
    // still sitting in a box on a screen somebody walks away from.
    vi.mocked(api.POST).mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 204 }),
    })

    await mount()

    await userEvent.type(screen.getByLabelText(/Current password/), 'the-old-password')
    await userEvent.type(screen.getByLabelText(/New password/), 'a-long-enough-password')
    await userEvent.click(screen.getByRole('button', { name: 'Change password' }))

    await waitFor(() => {
      expect(screen.getByLabelText(/Current password/)).toHaveValue('')
    })
    expect(screen.getByLabelText(/New password/)).toHaveValue('')
  }) // covers: AC-14

  it('sends the person to the sign in screen even when signing out fails', async () => {
    // A session that could not be ended is not a reason to leave somebody
    // standing on a signed in screen. The cookie is going either way.
    vi.mocked(api.POST).mockRejectedValue(new Error('the network is down'))

    const { router } = await mount()

    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }))

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(SIGN_IN_PATH)
    })
    expect(vi.mocked(api.POST)).toHaveBeenCalledWith('/api/auth/sign-out')
  }) // covers: AC-13

  it('redraws from a newer bundle without a reload', async () => {
    // The regression the identity hook exists for. `PATCH /api/me` answers with
    // a fresh bundle and writes it into the query; a screen reading the route
    // loader's snapshot instead kept the old one, so a change reached the cache
    // and no screen until somebody reloaded the page.
    //
    // The address rather than one of the form boxes, on purpose. Every input
    // here seeds its own state once and then belongs to whoever is typing in
    // it, which is right: a bundle arriving mid sentence must not rewrite the
    // word somebody is halfway through. What has to follow the identity is
    // what the screen simply states.
    const { queryClient } = await mount()
    expect(screen.getByText('ada@example.test')).toBeInTheDocument()

    const readdressed: Identity = {
      ...IDENTITY,
      staff: { ...IDENTITY.staff, email: 'ada.owner@example.test' },
    }

    act(() => {
      queryClient.setQueryData<Identity | null>(IDENTITY_KEY, readdressed)
    })

    await waitFor(() => {
      expect(screen.getByText('ada.owner@example.test')).toBeInTheDocument()
    })
  }) // covers: AC-16
})
