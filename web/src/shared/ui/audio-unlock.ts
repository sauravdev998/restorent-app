import { useSyncExternalStore } from 'react'

/**
 * Whether this browser has let us make a sound yet, and the sound itself.
 *
 * Every browser refuses to play audio until the person has interacted with the
 * page at least once. That is not a bug to work around, it is the rule, and it
 * means a waiter who loads the app and puts the phone in an apron without
 * tapping anything will get no sound at all.
 *
 * Which is why nothing in this platform ever depends on the sound. The ready
 * alert fires visually and through a live region first, always, and this file
 * only adds a chime on top when it can. Module scoped and not persisted: a new
 * tab starts locked again, because a new tab has had no interaction yet.
 */
let context: AudioContext | null = null

const UNLOCKING_EVENTS = ['pointerdown', 'keydown'] as const

type Listener = () => void

const listeners = new Set<Listener>()

/** True once the browser has allowed audio in this tab. */
export function isAudioUnlocked(): boolean {
  return context !== null
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/**
 * Whether audio is allowed, kept current as it changes.
 *
 * For a screen that shows something while audio is still locked. The plain
 * [`isAudioUnlocked`] reads a module flag, so a screen that calls it while
 * rendering is reading a value nothing tells it about: the first tap opens the
 * audio context and the screen carries on showing "sound is off" until some
 * unrelated change happens to redraw it. On a kitchen tablet nobody touches
 * that can be the rest of the shift.
 */
export function useAudioUnlocked(): boolean {
  return useSyncExternalStore(subscribe, isAudioUnlocked, () => false)
}

/**
 * The open audio context, or `null` while audio is still locked.
 *
 * For a surface that synthesises its own sound rather than playing the ready
 * chime: the kitchen's ticket chime is three descending knocks and has to be
 * unmistakably not this file's two rising notes. The context itself stays here,
 * opened once in the one gesture a browser allows it in, because two contexts in
 * one tab is a second thing that can be refused.
 */
export function audioContext(): AudioContext | null {
  return context
}

/**
 * Listens for the first interaction of the session and opens the audio context
 * inside it, which is the only moment a browser will allow it.
 *
 * Returns a function that removes the listeners.
 */
export function primeAudioUnlock(): () => void {
  if (typeof window === 'undefined' || typeof AudioContext === 'undefined') {
    return () => {
      // Nothing was attached, so there is nothing to remove.
    }
  }

  const remove = () => {
    for (const name of UNLOCKING_EVENTS) window.removeEventListener(name, unlock)
  }

  const unlock = () => {
    if (context) return
    try {
      context = new AudioContext()
      void context.resume()
    } catch {
      // An audio context this browser will not give us. The visual alert and
      // the live region announcement do not care, so neither do we.
      context = null
    }
    remove()
    for (const listener of listeners) listener()
  }

  for (const name of UNLOCKING_EVENTS) window.addEventListener(name, unlock)
  return remove
}

/**
 * Two short rising notes, meaning food is ready.
 *
 * Synthesised rather than loaded from a file, so there is no asset to ship, no
 * request to make on a restaurant's patchy wifi, and nothing to fail silently
 * at the moment it is needed. Does nothing at all if audio is still locked.
 */
export function playReadyChime(): void {
  if (!context) return

  const start = context.currentTime
  for (const [index, frequency] of [880, 1174.7].entries()) {
    const oscillator = context.createOscillator()
    const gain = context.createGain()

    oscillator.type = 'sine'
    oscillator.frequency.value = frequency

    const at = start + index * 0.16
    gain.gain.setValueAtTime(0.0001, at)
    gain.gain.exponentialRampToValueAtTime(0.2, at + 0.02)
    gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.15)

    oscillator.connect(gain).connect(context.destination)
    oscillator.start(at)
    oscillator.stop(at + 0.16)
  }
}
