import { audioContext } from '@/shared/ui/audio-unlock'

/**
 * The kitchen's own chime: three low knocks, meaning a ticket has arrived.
 *
 * **Deliberately unlike the waiter's ready chime**, which is two short rising
 * notes. The two sounds mean opposite things (work arriving versus work
 * finishing) and in a small restaurant they are audible from the same spot, so a
 * chef who hears the waiter's chime and looks at the pass, or a waiter who hears
 * the kitchen's and walks to collect nothing, is a real cost. Three differences
 * carry it, not one: the pitch is far lower, there are three notes rather than
 * two, and they descend rather than rise.
 *
 * Synthesised rather than loaded from a file, the same as the waiter's, so there
 * is no asset to ship and nothing to fail silently on a restaurant's patchy wifi
 * at the moment it is needed.
 *
 * Does nothing at all while audio is still locked, which is most of a kitchen
 * tablet's life. **Nothing on the pass may depend on this sound**: every alert it
 * accompanies is drawn on the screen first and separately.
 */
export function playKitchenChime(): void {
  const context = audioContext()
  if (!context) return

  const start = context.currentTime

  for (const [index, frequency] of [392, 329.6, 261.6].entries()) {
    const oscillator = context.createOscillator()
    const gain = context.createGain()

    // A triangle rather than the waiter's sine: more upper harmonics, so it
    // carries over an extractor fan running at full tilt.
    oscillator.type = 'triangle'
    oscillator.frequency.value = frequency

    const at = start + index * 0.2
    gain.gain.setValueAtTime(0.0001, at)
    gain.gain.exponentialRampToValueAtTime(0.28, at + 0.03)
    gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.19)

    oscillator.connect(gain).connect(context.destination)
    oscillator.start(at)
    oscillator.stop(at + 0.2)
  }
}
