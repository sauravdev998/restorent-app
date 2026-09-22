import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { isAudioUnlocked, primeAudioUnlock, useAudioUnlocked } from './audio-unlock'

/**
 * Whether a screen can be told that audio has been allowed.
 *
 * The flag itself always worked. What did not was anybody hearing about it: the
 * kitchen pass read it while rendering, so the first tap opened the audio
 * context and the "sound is off" prompt stayed up until something unrelated
 * redrew the screen. On a wall mounted tablet between tickets, that is the rest
 * of the shift.
 */

/** A stand in for the audio context, which jsdom does not provide. */
class FakeAudioContext {
  currentTime = 0
  destination = {}
  resume() {
    return Promise.resolve()
  }
  createOscillator() {
    return {
      type: '',
      frequency: { value: 0 },
      connect: () => ({}),
      start: () => {},
      stop: () => {},
    }
  }
  createGain() {
    return {
      gain: { setValueAtTime: () => {}, exponentialRampToValueAtTime: () => {} },
      connect: () => ({}),
    }
  }
}

function Screen() {
  const unlocked = useAudioUnlocked()
  return <p>{unlocked ? 'sound is on' : 'sound is off'}</p>
}

describe('audio unlock', () => {
  let remove: () => void

  beforeEach(() => {
    vi.stubGlobal('AudioContext', FakeAudioContext)
    remove = primeAudioUnlock()
  })

  afterEach(() => {
    remove()
    vi.unstubAllGlobals()
    vi.resetModules()
  })

  it('tells a screen the moment the first interaction allows it', async () => {
    render(<Screen />)
    expect(screen.getByText('sound is off')).toBeInTheDocument()

    // The one gesture a browser opens an audio context in.
    window.dispatchEvent(new Event('pointerdown'))

    expect(await screen.findByText('sound is on')).toBeInTheDocument()
    expect(isAudioUnlocked()).toBe(true)
  })
})
