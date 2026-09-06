import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'

import { directionOf, FALLBACK_LANGUAGE } from './catalogue'

/**
 * Keeps the document itself in step with the language on screen.
 *
 * Three things live outside React's tree and none of them update themselves.
 *
 * `lang` on the root element is what tells a screen reader which voice to read
 * the page in. Left saying `en` on a Hindi screen, a reader pronounces
 * Devanagari with English phonetics, which is not an accent but noise.
 *
 * `dir` comes from the catalogue rather than being assumed. Both shipped
 * languages are written left to right, so this changes nothing today and is the
 * reason adding a right to left language later is a catalogue entry rather than
 * a sweep across every screen.
 *
 * The tab title is text a person reads, so it is translated like all the rest.
 * `index.html` carries an English one for the moment before the app boots, and
 * this replaces it.
 *
 * Set here, once, rather than in each shell. The document element outlives every
 * component that might have set it, so an effect that runs per surface would
 * leave the last surface's language on a page that has moved on.
 */
export function useDocumentLanguage(): void {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage ?? FALLBACK_LANGUAGE

  useEffect(() => {
    const root = document.documentElement
    root.lang = language
    root.dir = directionOf(language)
  }, [language])

  useEffect(() => {
    document.title = t('app.name')
  }, [t, language])
}
