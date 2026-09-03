import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { countries } from '@/shared/countries'
import { expectAccessible } from '@/test/axe'

import { Register } from './register'
import { SignIn } from './sign-in'

/**
 * The typed client, replaced wholesale.
 *
 * The real one builds a `Request` from a relative address, which a browser
 * resolves against the page it is on and which jsdom refuses outright. So there
 * is no way to drive these screens through the real client under test, and a
 * stubbed `fetch` would only move the failure one layer along. What is under
 * test here is what a screen does with an answer, which this hands it directly.
 */
vi.mock('@/shared/api/client', () => ({
  api: { GET: vi.fn(), POST: vi.fn(), PATCH: vi.fn() },
  handleSignedOut: vi.fn(),
}))

/**
 * The two screens that render outside the application shell.
 *
 * They are tested together because what makes them a pair is exactly what is
 * being checked: neither has the shell around it, so each has to provide for
 * itself the landmark, the heading, and the language switcher the shell would
 * otherwise have provided. A third screen outside the shell means writing those
 * three again, and only a page level render catches a missing one.
 */

function mount(element: React.ReactElement, path = '/') {
  const router = createMemoryRouter([{ path: '/', element }], { initialEntries: [path] })

  return (
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}

/** Answers the next call the way the API answers a failed one. */
function answerWith(
  status: number,
  body: { error: string; message: string; fields?: Record<string, string> },
) {
  vi.mocked(api.POST).mockResolvedValue({
    error: body,
    response: new Response(null, { status }),
  })
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('the screens outside the shell', () => {
  it('are accessible as whole pages, landmarks and headings included', async () => {
    await expectAccessible(mount(<SignIn />), { page: true })
    await expectAccessible(mount(<Register />), { page: true })
  }) // covers: AC-21

  it('each carry their own main, heading, and language switcher', () => {
    for (const [name, element] of [
      ['sign in', <SignIn key="in" />],
      ['register', <Register key="up" />],
    ] as const) {
      const view = render(mount(element))

      expect(view.getByRole('main'), `${name} has no main landmark`).toBeInTheDocument()
      expect(
        view.getAllByRole('heading', { level: 1 }),
        `${name} has no level one heading`,
      ).toHaveLength(1)
      // Somebody who cannot read the current language has to be able to change
      // it before they sign in, which is the one moment the shell's switcher is
      // not on screen.
      expect(
        view.getByRole('combobox', { name: 'Language' }),
        `${name} offers no way to change language`,
      ).toBeInTheDocument()

      view.unmount()
    }
  }) // covers: AC-21
})

describe('SignIn', () => {
  it('says one thing about a refused sign in, and never which half was wrong', async () => {
    // The server answers a wrong password and an address nobody has
    // identically, on purpose. A screen that put the message beside the address
    // box would undo that by saying the address was the part that failed.
    answerWith(401, { error: 'unauthenticated', message: 'no' })

    render(mount(<SignIn />))

    await userEvent.type(screen.getByLabelText(/Email address/), 'ada@example.test')
    await userEvent.type(screen.getByLabelText(/Password/), 'a-real-password')
    await userEvent.click(screen.getByRole('button', { name: 'Sign in' }))

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('do not match an account')

    // One message, not two, and neither control is marked invalid.
    expect(screen.getAllByRole('alert')).toHaveLength(1)
    expect(screen.getByLabelText(/Email address/)).not.toHaveAttribute('aria-invalid')
    expect(screen.getByLabelText(/Password/)).not.toHaveAttribute('aria-invalid')
  }) // covers: AC-4

  it('says the service is unwell rather than blaming the password', async () => {
    // Telling somebody their password is wrong when the database is down sends
    // them off to reset a password that was fine.
    answerWith(503, { error: 'unavailable', message: 'no' })

    render(mount(<SignIn />))

    await userEvent.type(screen.getByLabelText(/Email address/), 'ada@example.test')
    await userEvent.type(screen.getByLabelText(/Password/), 'a-real-password')
    await userEvent.click(screen.getByRole('button', { name: 'Sign in' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('not answering')
  }) // covers: AC-4
})

describe('Register', () => {
  it('offers exactly the countries the API compiles in, and no others', () => {
    render(mount(<Register />))

    const options = screen
      .getAllByRole('option')
      // The language switcher has options of its own on this screen.
      .filter((option) => countries.some((country) => country.englishName === option.textContent))

    expect(options).toHaveLength(countries.length)
    expect(options.length).toBeGreaterThan(0)
  }) // covers: AC-1

  it('puts each refused field’s reason beside the field it belongs to', async () => {
    // The other half of the sign in rule. A refused registration is not one
    // opaque failure: it is a form, and somebody has to be able to fix it.
    answerWith(400, {
      error: 'invalid',
      message: 'no',
      fields: { email: 'already_taken', password: 'too_short' },
    })

    render(mount(<Register />))

    await userEvent.type(screen.getByLabelText(/Restaurant name/), 'The Test Kitchen')
    await userEvent.type(screen.getByLabelText(/Your name/), 'Ada Owner')
    await userEvent.type(screen.getByLabelText(/Email address/), 'ada@example.test')
    await userEvent.type(screen.getByLabelText(/^Password/), 'short')
    await userEvent.click(screen.getByRole('button', { name: 'Register' }))

    const email = await screen.findByLabelText(/Email address/)
    expect(email).toHaveAttribute('aria-invalid', 'true')
    expect(screen.getByLabelText(/^Password/)).toHaveAttribute('aria-invalid', 'true')

    // And the words are this side's, translated with everything else, rather
    // than the API's English `message`.
    expect(screen.getByText('That is already taken.')).toBeInTheDocument()
    expect(screen.getByText('That is too short.')).toBeInTheDocument()
    expect(screen.queryByText('no')).not.toBeInTheDocument()
  }) // covers: AC-2, AC-21
})
