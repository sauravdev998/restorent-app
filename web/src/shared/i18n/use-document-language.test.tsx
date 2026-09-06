import { act, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { directionOf, FALLBACK_LANGUAGE } from './catalogue'
import i18next, { changeLanguage } from './index'
import { useDocumentLanguage } from './use-document-language'

/**
 * The three things that live outside React's tree and none of which update
 * themselves: `lang` and `dir` on the root element, and the tab title.
 *
 * `lang` is the one that matters most and is the easiest to leave stale. Left
 * saying `en` on a Hindi screen, a screen reader pronounces Devanagari with
 * English phonetics, which is not an accent but noise.
 */

/** A screen that does nothing but keep the document in step. */
function Screen() {
  useDocumentLanguage()
  return <h1>A screen</h1>
}

afterEach(async () => {
  await act(async () => {
    await changeLanguage(FALLBACK_LANGUAGE, 'admin')
  })
})

describe('useDocumentLanguage', () => {
  it('marks the document as the language on screen', () => {
    render(<Screen />)

    expect(document.documentElement.lang).toBe(i18next.resolvedLanguage)
    expect(document.documentElement.lang).toBe('en')
  }) // covers: AC-10

  it('sets the direction from the catalogue rather than assuming one', () => {
    render(<Screen />)

    const language = i18next.resolvedLanguage ?? FALLBACK_LANGUAGE
    expect(document.documentElement.dir).toBe(directionOf(language))
  }) // covers: AC-10

  it('replaces the English title index.html carries before the app boots', () => {
    document.title = 'something stale'

    render(<Screen />)

    expect(document.title).toBe(i18next.t('app.name'))
    expect(document.title).toBe('Restaurant operations')
  }) // covers: AC-10

  it('follows a language change without the page reloading', async () => {
    render(<Screen />)

    await act(async () => {
      await changeLanguage('hi', 'admin')
    })

    expect(document.documentElement.lang).toBe('hi')
    // The tab title is text a person reads, so it is translated with the rest
    // rather than left in English.
    expect(document.title).toBe('रेस्टोरेंट संचालन')
    expect(document.title).not.toBe('Restaurant operations')
  }) // covers: AC-8, AC-10

  it('keeps the direction right for the new language too', async () => {
    render(<Screen />)

    await act(async () => {
      await changeLanguage('hi', 'admin')
    })

    expect(document.documentElement.dir).toBe(directionOf('hi'))
    expect(document.documentElement.dir).toBe('ltr')
  }) // covers: AC-10

  it('goes back when the language goes back', async () => {
    render(<Screen />)

    await act(async () => {
      await changeLanguage('hi', 'admin')
    })
    await act(async () => {
      await changeLanguage('en', 'admin')
    })

    expect(document.documentElement.lang).toBe('en')
    expect(document.title).toBe('Restaurant operations')
  }) // covers: AC-10
})
