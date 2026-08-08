import i18next from 'i18next'
import LanguageDetector from 'i18next-browser-languagedetector'
import { initReactI18next } from 'react-i18next'

import en from '@/locales/en/common.json'

/**
 * Translations, set up before the first screen renders.
 *
 * Feature 6 (language and text foundation) owns this properly, including how a
 * staff member's choice is stored and how files are loaded on demand. It is
 * wired now because retrofitting translations across twenty screens is painful
 * and doing it from the first screen costs nothing.
 *
 * The rule from here on: no user facing string is written into a component.
 */
export const DEFAULT_LANGUAGE = 'en'

await i18next
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { common: en },
    },
    fallbackLng: DEFAULT_LANGUAGE,
    defaultNS: 'common',
    interpolation: {
      // React escapes for us, so i18next doing it again would double escape.
      escapeValue: false,
    },
  })

export default i18next
