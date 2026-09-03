import { describe, expect, it } from 'vitest'

import {
  isPublicPath,
  landingFor,
  REGISTER_PATH,
  returnPathFrom,
  SIGN_IN_PATH,
  signInPathFor,
} from './signed-out'

/**
 * What happens when the server says nobody is signed in.
 *
 * One path, reached three ways: an ordinary request that came back `401`, the
 * live stream failing fatally, and a screen loaded while already signed out.
 * The parts worth testing are the two that are easy to get quietly wrong: where
 * somebody is sent back to, and where a role lands.
 */

describe('signInPathFor', () => {
  it('preserves the path so signing back in returns somebody to their work', () => {
    expect(signInPathFor('/waiter', '?table=7')).toBe(
      `${SIGN_IN_PATH}?next=${encodeURIComponent('/waiter?table=7')}`,
    )
  }) // covers: AC-19

  it('does not send somebody back to the screen they just signed in on', () => {
    // The loop a naive "remember the last path" makes: signed out on the sign
    // in screen, sent to the sign in screen, told to return to the sign in
    // screen.
    expect(signInPathFor(SIGN_IN_PATH)).toBe(SIGN_IN_PATH)
    expect(signInPathFor(REGISTER_PATH)).toBe(SIGN_IN_PATH)

    expect(isPublicPath(SIGN_IN_PATH)).toBe(true)
    expect(isPublicPath('/admin')).toBe(false)
  }) // covers: AC-19
})

describe('returnPathFrom', () => {
  it('returns somebody to the path a 401 preserved', () => {
    const search = `?next=${encodeURIComponent('/waiter?table=7')}`
    expect(returnPathFrom(search, '/admin')).toBe('/waiter?table=7')
  }) // covers: AC-19

  it('falls back to the role’s own landing when nothing was preserved', () => {
    expect(returnPathFrom('', '/kitchen')).toBe('/kitchen')
    expect(returnPathFrom('?other=1', '/kitchen')).toBe('/kitchen')
  }) // covers: AC-20

  it('refuses a next that is not a path on this site', () => {
    // It arrives from the address bar, so it is somebody's input. Without this
    // check, a link to `/sign-in?next=https://elsewhere.example` walks a person
    // who signed in on our own screen straight off it, with our name on the
    // page they left from.
    for (const hostile of [
      'https://elsewhere.example',
      '//elsewhere.example',
      'javascript:alert(1)',
      'waiter',
    ]) {
      expect(returnPathFrom(`?next=${encodeURIComponent(hostile)}`, '/admin')).toBe('/admin')
    }
  }) // covers: AC-19
})

describe('landingFor', () => {
  it('sends each role to the surface it holds', () => {
    expect(landingFor('admin')).toBe('/admin')
    expect(landingFor('waiter')).toBe('/waiter')
    // `chef` is the role; `kitchen` is the surface. The two words differ on
    // purpose and this is the one place they have to be joined up.
    expect(landingFor('chef')).toBe('/kitchen')
  }) // covers: AC-20

  it('sends a role it has never heard of somewhere that renders', () => {
    // A role added to the API and not here must not produce an empty address.
    expect(landingFor('sommelier')).toBe('/')
  }) // covers: AC-20
})
