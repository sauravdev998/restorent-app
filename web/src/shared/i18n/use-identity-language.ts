import { useEffect } from 'react'

import type { Identity } from '@/shared/session/identity'
import type { Surface } from '@/shared/surface'

import { changeLanguage } from './index'
import { resolveLanguage } from './resolve'

/**
 * Keeps the interface in the language the signed in person should be reading.
 *
 * The app boots before it knows who is looking. The sign in screen has to be
 * readable, and the only thing available then is whatever somebody explicitly
 * chose on this device, so that is what `i18n/index.ts` opens in. This hook is
 * the other half: once the identity arrives, or changes, the screen moves to the
 * language that person's own setting and their restaurant's default resolve to.
 *
 * It also runs on a surface change, because the kitchen follows the restaurant
 * while the other two follow the person. Walking from the waiter screen to the
 * kitchen on a shared tablet has to change the language, and walking back has to
 * change it again.
 *
 * A failed switch changes nothing at all. `changeLanguage` fetches the files
 * before it moves, so a request that fails on a phone with one bar leaves the
 * screen exactly as it was rather than half translated. There is nothing to say
 * about it here: nobody asked for this switch, so nobody is waiting on a toast.
 */
export function useIdentityLanguage(identity: Identity | null, surface: Surface): void {
  useEffect(() => {
    void changeLanguage(resolveLanguage(identity, surface), surface).catch(() => {
      console.error('Could not load the translations for the signed in language.')
    })
  }, [identity, surface])
}
