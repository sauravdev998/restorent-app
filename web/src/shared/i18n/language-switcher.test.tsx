import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { expectAccessible } from '@/test/axe'

import { catalogue, FALLBACK_LANGUAGE } from './catalogue'
import i18next, { changeLanguage } from './index'
import { LanguageSwitcher } from './language-switcher'
import { PSEUDO_LANGUAGE, PSEUDO_LANGUAGE_NAME } from './pseudo'
import { LANGUAGE_STORAGE_KEY } from './resolve'

/**
 * Picking the language the interface is drawn in.
 *
 * The switch itself is mocked in one direction only: the real
 * `changeLanguage` is what runs everywhere except the failure test, which needs
 * a load that fails and cannot get one from files that are all present. What
 * that test then checks is the promise this component makes, which is that a
 * failed switch changes nothing at all.
 */
vi.mock('./index', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./index')>()
  return { ...actual, changeLanguage: vi.fn(actual.changeLanguage) }
})

const switching = vi.mocked(changeLanguage)

/** The toasts raised while a body runs, which is how a failure is reported. */
async function toastsDuring(body: () => Promise<void>): Promise<string[]> {
  const store = await import('@/shared/ui/toast-store')
  const raised: string[] = []
  const stop = store.subscribeToToasts((toasts) => {
    raised.push(...toasts.map((toast) => toast.title))
  })

  try {
    await body()
  } finally {
    stop()
  }

  return raised
}

beforeEach(() => {
  window.localStorage.clear()
  switching.mockClear()
})

/**
 * Back to English between tests, and never by replacing the mock's own body.
 * Every override below is a `...Once`, so the real switch stays underneath and
 * this line performs a real one.
 */
afterEach(async () => {
  await act(async () => {
    await switching(FALLBACK_LANGUAGE, 'admin')
  })
  switching.mockClear()
  window.localStorage.clear()
})

describe('LanguageSwitcher', () => {
  it('is a named control, so nobody has to guess what the list is for', () => {
    render(<LanguageSwitcher surface="waiter" />)

    expect(screen.getByRole('combobox', { name: 'Language' })).toBeInTheDocument()
  }) // covers: AC-1, AC-17

  it('writes every language in its own script rather than translating the list', () => {
    render(<LanguageSwitcher surface="waiter" />)

    // Somebody looking for Hindi is by definition somebody who may not be able
    // to read the word "Hindi" in the language currently on screen.
    for (const language of catalogue.languages) {
      expect(screen.getByRole('option', { name: language.nativeName })).toBeInTheDocument()
    }
    expect(screen.getByRole('option', { name: 'हिन्दी' })).toBeInTheDocument()
  }) // covers: AC-2

  it('marks each option with its own language, so a reader announces it right', () => {
    render(<LanguageSwitcher surface="waiter" />)

    // The list is a mix of scripts by design, and a reader announcing हिन्दी
    // with English phonetics defeats the point of showing it in its own script.
    expect(screen.getByRole('option', { name: 'हिन्दी' })).toHaveAttribute('lang', 'hi')
    expect(screen.getByRole('option', { name: 'English' })).toHaveAttribute('lang', 'en')
  }) // covers: AC-11

  it('shows the language currently on screen as the selected one', () => {
    render(<LanguageSwitcher surface="waiter" />)

    const select = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Language' })
    expect(select.value).toBe(i18next.resolvedLanguage ?? FALLBACK_LANGUAGE)
  }) // covers: AC-10

  it('offers the fake language while developing', () => {
    render(<LanguageSwitcher surface="waiter" />)

    const pseudo = screen.getByRole<HTMLOptionElement>('option', { name: PSEUDO_LANGUAGE_NAME })

    expect(pseudo.value).toBe(PSEUDO_LANGUAGE)
    // Named in the fallback language, because it has no script of its own and
    // the only person who ever sees it is an engineer looking for it.
    expect(pseudo).toHaveAttribute('lang', FALLBACK_LANGUAGE)
  }) // covers: AC-16

  it('switches the interface and remembers the choice on this device', async () => {
    const user = userEvent.setup()
    render(<LanguageSwitcher surface="waiter" />)

    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'hi')

    // Waited for rather than asserted straight away, and the waiting is the
    // behaviour: the files are fetched before the language moves, so the switch
    // is deliberately not instant.
    await waitFor(() => {
      expect(i18next.resolvedLanguage).toBe('hi')
    })
    // Remembered only as the seed for the next sign in screen on this device.
    // The signed in resolver never reads it back.
    expect(window.localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe('hi')
  }) // covers: AC-7, AC-8

  it('switches back again, so the choice is not one way', async () => {
    const user = userEvent.setup()
    render(<LanguageSwitcher surface="waiter" />)
    const select = screen.getByRole('combobox', { name: 'Language' })

    await user.selectOptions(select, 'hi')
    await waitFor(() => {
      expect(i18next.resolvedLanguage).toBe('hi')
    })

    await user.selectOptions(select, 'en')
    await waitFor(() => {
      expect(i18next.resolvedLanguage).toBe('en')
    })

    expect(window.localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe('en')
  }) // covers: AC-7, AC-8

  it('fetches only the namespaces the surface it sits on needs', async () => {
    const user = userEvent.setup()
    render(<LanguageSwitcher surface="waiter" />)

    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'hi')

    // The surface goes with the request, which is what stops a switch on the
    // waiter screen waiting on the admin file.
    expect(switching).toHaveBeenCalledWith('hi', 'waiter')
  }) // covers: AC-5

  it('changes nothing at all when the translations fail to load', async () => {
    const user = userEvent.setup()
    const before = i18next.resolvedLanguage
    switching.mockRejectedValueOnce(new Error('the network went away'))

    render(<LanguageSwitcher surface="waiter" />)

    const raised = await toastsDuring(async () => {
      await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'hi')
    })

    // A screen half in one language and half in another is worse than a screen
    // that stayed as it was, so all three of these hold together or none do.
    expect(i18next.resolvedLanguage).toBe(before)
    expect(window.localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBeNull()
    expect(raised).toContain('Could not switch language')
  }) // covers: AC-9

  it('says the switch failed in the language still on screen', async () => {
    const user = userEvent.setup()
    switching.mockRejectedValueOnce(new Error('the network went away'))

    render(<LanguageSwitcher surface="waiter" />)

    const raised = await toastsDuring(async () => {
      await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'hi')
    })

    // The one the person can read, which is the point of raising it after the
    // failure rather than before the attempt.
    expect(raised).toContain(i18next.t('language.failed'))
    expect(raised).not.toContain('language.failed')
  }) // covers: AC-3, AC-9

  it('stays available after a failed switch, so it can be tried again', async () => {
    const user = userEvent.setup()
    switching.mockRejectedValueOnce(new Error('the network went away'))

    render(<LanguageSwitcher surface="waiter" />)
    const select = screen.getByRole('combobox', { name: 'Language' })

    await user.selectOptions(select, 'hi')

    expect(select).toBeEnabled()
  }) // covers: AC-9

  it('refuses a second switch while one is still in flight', async () => {
    const user = userEvent.setup()

    // A switch that has not answered yet, which is every switch on a phone with
    // one bar. Two of them at once would race, and the loser would win.
    let release: (() => void) | undefined
    switching.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          release = resolve
        }),
    )

    render(<LanguageSwitcher surface="waiter" />)
    const select = screen.getByRole('combobox', { name: 'Language' })

    await user.selectOptions(select, 'hi')

    expect(select).toBeDisabled()

    await act(async () => {
      release?.()
      // Let the component's own `finally` run before asking again.
      await Promise.resolve()
    })

    expect(select).toBeEnabled()
  }) // covers: AC-9

  it('is accessible in every room and both appearances', async () => {
    await expectAccessible(<LanguageSwitcher surface="admin" />)
  }) // covers: AC-1
})
