import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from '@/shared/api/client'
import { countries } from '@/shared/countries'
import { FALLBACK_LANGUAGE } from '@/shared/i18n/catalogue'
import { changeLanguage } from '@/shared/i18n'
import { LANGUAGE_STORAGE_KEY, writeStoredLanguage } from '@/shared/i18n/resolve'
import { DEFAULT_SURFACE } from '@/shared/surface'
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

// Two things to put back, and the second one is the interesting one.
//
// i18next is one instance for the whole file, so a test that switches language
// and leaves it switched is every later test hunting for English words on a
// Hindi screen. Restored here rather than in the test that switches, because
// the switch resolves after the assertion that wanted it.
//
// And the device's own remembered choice, because these screens read it back on
// every mount. That is the behaviour, not a leak: a house phone opens in the
// language the last person chose on it. It does mean a test that switches has
// to hand the device back as it found it.
afterEach(async () => {
  window.localStorage.removeItem(LANGUAGE_STORAGE_KEY)
  await act(async () => {
    await changeLanguage(FALLBACK_LANGUAGE, DEFAULT_SURFACE)
  })
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

  it('offer the skip link the shell gives every other screen', () => {
    for (const [name, element] of [
      ['sign in', <SignIn key="in" />],
      ['register', <Register key="up" />],
    ] as const) {
      const view = render(mount(element))

      // Both screens always had the `main` to skip to and neither had the link
      // that skips to it, which is the half a keyboard user actually presses.
      const skip = view.getByRole('link', { name: 'Skip to main content' })

      expect(skip, `${name} has no skip link`).toBeInTheDocument()
      expect(skip.getAttribute('href'), `${name}'s skip link points nowhere`).toBe('#main-content')
      expect(view.getByRole('main').id, `${name}'s skip link has nothing to land on`).toBe(
        'main-content',
      )

      // First in the tab order, or it is not a skip link: a link the reader
      // reaches after the form has skipped nothing.
      expect(view.container.querySelector('a')).toBe(skip)

      view.unmount()
    }
  }) // covers: AC-21

  it('mark the document as the language they are actually drawn in', async () => {
    const view = render(mount(<SignIn />))

    expect(document.documentElement.lang).toBe('en')

    await userEvent.selectOptions(view.getByRole('combobox', { name: 'Language' }), 'hi')

    // Waited for rather than asserted straight away: switching fetches the
    // language's files before it moves, so the screen is still English for a
    // tick afterwards by design.
    //
    // The regression: these two screens render outside the shell, and the
    // shell was the only caller of the hook that writes `lang`. So Devanagari
    // was served under `lang="en"` and a screen reader read it with English
    // phonetics, which is noise rather than an accent.
    await waitFor(() => {
      expect(document.documentElement.lang).toBe('hi')
    })

    view.unmount()
  }) // covers: AC-21

  it('open in the language this device last chose, not the last person\u2019s', async () => {
    // What a shared house phone in a Hindi speaking restaurant needs. Signing
    // in moves the screen to the person's own language, and signing out has to
    // hand the device back the way its owner set it.
    //
    // The regression: nothing re resolved the language once the shell
    // unmounted, so after an English speaker's shift the sign in screen stayed
    // English until somebody reloaded the page, however the phone was set.
    writeStoredLanguage('hi')

    const view = render(mount(<SignIn />))

    await waitFor(() => {
      expect(view.getByRole('heading', { level: 1 })).toHaveTextContent(
        '\u0938\u093e\u0907\u0928 \u0907\u0928',
      )
    })
    expect(document.documentElement.lang).toBe('hi')

    view.unmount()
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
