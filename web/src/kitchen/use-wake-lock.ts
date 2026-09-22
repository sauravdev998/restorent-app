import { useEffect } from 'react'

/**
 * Holds the tablet's screen awake while the pass is open, and lets go when the
 * chef leaves it.
 *
 * A kitchen pass is a screen nobody touches for an hour at a time and everybody
 * needs to be able to glance at. Left to the device it dims, then sleeps, and the
 * one screen in the building that must always be readable is the one showing a
 * black rectangle.
 *
 * **Three things about it are deliberate.**
 *
 * It is released on unmount, not left held. The lock belongs to the pass, not to
 * the app: a chef who walks to the Menu tab, or signs out, should not leave the
 * tablet burning its screen on nothing.
 *
 * It is re requested when the tab becomes visible again. Browsers drop a wake
 * lock whenever the page is hidden and never hand it back on their own, so
 * without this the first time somebody switches tabs is the last time the pass
 * stays awake.
 *
 * **Every failure is swallowed on purpose.** Several browsers have no wake lock
 * at all, some refuse it outside a secure context, and some reject the promise
 * for reasons nothing here can act on. The fallback is the device's own screen
 * timeout, which somebody sets at install time, and a pass that threw an error
 * screen because it could not dim less would be far worse than one that dims.
 */
export function useWakeLock(): void {
  useEffect(() => {
    // Not in the DOM types this project compiles against, and not in jsdom
    // either, so it is reached through a narrow shape rather than a cast of the
    // whole navigator.
    const locks = (
      navigator as Navigator & {
        wakeLock?: { request: (type: 'screen') => Promise<WakeLockHandle> }
      }
    ).wakeLock

    if (!locks) return

    let held: WakeLockHandle | null = null
    let dropped = false

    const take = async (): Promise<void> => {
      if (dropped || held || document.visibilityState !== 'visible') return
      try {
        held = await locks.request('screen')
        // Released while the request was in flight, which happens when a chef
        // taps through to another screen immediately.
        if (dropped) {
          void held.release()
          held = null
        }
      } catch {
        // A browser that will not hold the screen awake. The device's own
        // timeout takes over and the pass carries on working.
        held = null
      }
    }

    const onVisibilityChange = (): void => {
      if (document.visibilityState === 'visible') void take()
      else held = null
    }

    void take()
    document.addEventListener('visibilitychange', onVisibilityChange)

    return () => {
      dropped = true
      document.removeEventListener('visibilitychange', onVisibilityChange)
      if (held) void held.release()
      held = null
    }
  }, [])
}

/** The part of a `WakeLockSentinel` this hook uses. */
interface WakeLockHandle {
  release: () => Promise<void>
}
