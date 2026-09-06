import { describe, expect, it } from 'vitest'

import { isKnownLanguage } from './catalogue'
import { PSEUDO_LANGUAGE, PSEUDO_LANGUAGE_NAME, pseudoAvailable, pseudoBundle } from './pseudo'

/**
 * The fake language, which exists to make two problems visible.
 *
 * A string that is not coming from a translation file stays plain English on a
 * pseudo screen while everything around it is mangled, and a layout that only
 * fits English breaks under the padding. Both only work if the mangling is
 * total and the padding is real, which is what these check.
 */

/** Every character that is not part of an interpolation placeholder. */
function withoutPlaceholders(value: string): string {
  return value.replace(/\{\{[^}]*\}\}/g, '')
}

describe('pseudoBundle', () => {
  it('brackets every string, so an unmangled one is obvious', () => {
    const bundle = pseudoBundle({ send: 'Send the round' })

    expect(bundle['send']).toMatch(/^\[.*\]$/)
  }) // covers: AC-16

  it('mangles every letter of the alphabet, in both cases', () => {
    // The whole method rests on this. One letter left alone is a word that
    // still reads as English, and an engineer scanning for English text then
    // chases a string that went through t() after all.
    const letters = 'abcdefghijklmnopqrstuvwxyz'
    const source = Object.fromEntries(
      [...letters, ...letters.toUpperCase()].map((letter) => [letter, letter]),
    )

    const bundle = pseudoBundle(source)

    for (const [letter, mangled] of Object.entries(bundle)) {
      expect(mangled).not.toBe(`[${letter}]`)
      expect(typeof mangled).toBe('string')
    }
  }) // covers: AC-16

  it('leaves an interpolation placeholder exactly as it was', () => {
    // `{{count}}` is not a word. Mangling it breaks the interpolation rather
    // than testing it, and a screen full of {{ćóúńt́}} teaches nobody anything.
    const bundle = pseudoBundle({ orders: '{{count}} orders waiting' })

    expect(bundle['orders']).toContain('{{count}}')
  }) // covers: AC-16

  it('leaves every placeholder in a string that has several', () => {
    const bundle = pseudoBundle({ sorted: 'Sorted by {{column}}, {{direction}}' })
    const mangled = bundle['sorted']

    expect(mangled).toContain('{{column}}')
    expect(mangled).toContain('{{direction}}')
  }) // covers: AC-16

  it('mangles the words around a placeholder rather than skipping the string', () => {
    const bundle = pseudoBundle({ orders: '{{count}} orders' })
    const mangled = bundle['orders']

    expect(typeof mangled).toBe('string')
    // The word survives as a word, and none of it is plain English any more.
    expect(withoutPlaceholders(mangled as string)).not.toContain('orders')
  }) // covers: AC-16

  it('pads by about a third, because real translations run longer', () => {
    // A button that fits its label exactly today is a button that wraps in
    // German. The padding is what makes that show up here rather than in a
    // screenshot from a restaurant.
    const english = 'Send the round'
    const bundle = pseudoBundle({ send: english })
    const mangled = bundle['send']

    const padding = [...(mangled as string)].filter((character) => character === '·').length
    expect(padding).toBe(Math.ceil(english.length * 0.35))
    expect(padding).toBeGreaterThan(0)
  }) // covers: AC-16

  it('pads a short string too, so nothing escapes by being small', () => {
    const bundle = pseudoBundle({ ok: 'OK' })

    expect(bundle['ok']).toContain('·')
  }) // covers: AC-16

  it('walks into nested groups, however deep somebody nested them', () => {
    const bundle = pseudoBundle({
      nav: { admin: 'Admin', bills: { open: 'Open bills' } },
    })

    const nav = bundle['nav']
    expect(typeof nav).toBe('object')

    const flat = JSON.stringify(bundle)
    expect(flat).not.toContain('"Admin"')
    expect(flat).not.toContain('"Open bills"')
    // The keys are the contract with every call site, so they are untouched.
    expect(flat).toContain('"admin"')
    expect(flat).toContain('"open"')
  }) // covers: AC-16

  it('leaves the keys alone while mangling the values', () => {
    const bundle = pseudoBundle({ 'language.label': 'Language' })

    expect(Object.keys(bundle)).toEqual(['language.label'])
  }) // covers: AC-16

  it('handles a bundle with nothing in it', () => {
    expect(pseudoBundle({})).toEqual({})
  }) // covers: AC-16

  it('carries a script that has no accents through without corrupting it', () => {
    // Hindi is never the source of the pseudo bundle, but a translator can put
    // Devanagari in an English file, and mangling must not mean mojibake.
    const bundle = pseudoBundle({ dish: 'मटर पनीर' })

    expect(bundle['dish']).toContain('मटर पनीर')
  }) // covers: AC-16
})

describe('the fake language itself', () => {
  it('is not offered as a real language', () => {
    // Kept out of the catalogue on purpose. The catalogue is the list both this
    // app and the API trust, and a code with no files behind it must never be
    // storable against a restaurant or a member of staff.
    expect(isKnownLanguage(PSEUDO_LANGUAGE)).toBe(false)
  }) // covers: AC-16

  it('is named in plain English, because only an engineer looks for it', () => {
    expect(PSEUDO_LANGUAGE_NAME).toBe('Pseudo (development)')
  }) // covers: AC-16

  it('is gated on the development flag rather than always on', () => {
    // The gate is what a production build drops the whole generator behind, so
    // it must read the flag rather than a constant somebody set by hand.
    expect(pseudoAvailable).toBe(import.meta.env.DEV)
  }) // covers: AC-16
})
