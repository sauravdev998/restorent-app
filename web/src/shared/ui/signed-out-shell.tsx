import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { LanguageSwitcher } from '@/shared/i18n/language-switcher'
import { useDocumentLanguage } from '@/shared/i18n/use-document-language'
import { useIdentityLanguage } from '@/shared/i18n/use-identity-language'
import { DEFAULT_SURFACE } from '@/shared/surface'

/** What a signed out screen puts inside the frame. */
export interface SignedOutShellProps {
  /** The screen itself, which becomes the contents of `main`. */
  children: ReactNode
}

/**
 * The frame the sign in and registration screens sit inside.
 *
 * These two are the only screens anybody reaches without a session, so they
 * render outside `SurfaceShell` and outside the route that resolves an identity.
 * That is what makes them easy to leave half finished: everything the shell does
 * for every other screen, these two have to do for themselves, and nothing goes
 * red when they do not.
 *
 * Four things, and they are here rather than copied into both screens.
 *
 * **The skip link.** Both screens already carried a `main` to skip to and
 * neither had the link. Small, but it is the first thing on the page for the
 * one person who needs it, and the rule spec 0004 set does not stop at the
 * shell's edge.
 *
 * **The document's language.** `lang` on the root element is set by
 * `useDocumentLanguage`, which used to be called only from `RootLayout`. So a
 * sign in screen switched to Hindi rendered Devanagari under `lang="en"` and a
 * screen reader read it with English phonetics, which is the exact failure that
 * hook exists to prevent.
 *
 * **Re resolving the language on the way out.** Signing out unmounts the shell,
 * and nothing was left to ask what a device with nobody signed in should read.
 * The screen simply kept whatever language the person who just left was using,
 * so a house phone set to Hindi came back in English after an English speaker's
 * shift, until somebody reloaded the page. `useIdentityLanguage(null, ...)`
 * resolves the signed out case, which is this device's own remembered choice.
 *
 * **The header.** One place that decides a signed out screen shows the product's
 * name and a language switcher, rather than two places that agree today.
 */
export function SignedOutShell({ children }: SignedOutShellProps) {
  const { t } = useTranslation()

  // Nobody is signed in on these two screens, and that is not a placeholder: it
  // is the case the resolver handles by reading this device's remembered
  // choice. Passing the identity would be wrong here even if there were one.
  useIdentityLanguage(null, DEFAULT_SURFACE)
  useDocumentLanguage()

  return (
    <div className="flex min-h-screen flex-col bg-background">
      {/* Same treatment as the shell's: off screen until it takes focus. */}
      <a
        href="#main-content"
        className="fixed top-0 start-4 z-50 -translate-y-full rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground focus:translate-y-4 print:hidden"
      >
        {t('a11y.skipToMain')}
      </a>

      <header className="flex items-center justify-between px-4 py-3">
        <span className="text-base font-semibold text-foreground">{t('app.name')}</span>
        {/* Signed out, so there is no personal setting to read. The switcher
            writes to this device, which is what the next person to pick this
            phone up will open the screen in. */}
        <LanguageSwitcher surface={DEFAULT_SURFACE} />
      </header>

      {/* `tabIndex={-1}` is what makes the skip link actually skip. Following a
          fragment link moves where the next Tab starts, but it only moves focus
          itself if the target can hold focus, and a `<main>` cannot by default.
          Without it, Safari and several screen readers leave focus on the body
          and the link does nothing for exactly the people it exists for. */}
      <main id="main-content" tabIndex={-1} className="flex flex-1 items-center justify-center p-4">
        {children}
      </main>
    </div>
  )
}
