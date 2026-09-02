/** How loudly to say something. `assertive` interrupts; use it sparingly. */
export type Urgency = 'polite' | 'assertive'

type Listener = (message: string, urgency: Urgency) => void

const listeners = new Set<Listener>()

/**
 * Says something out loud, through the one live region mounted in the shell.
 *
 * Every announcement in the platform goes through here. Toasts and alerts do
 * not each own a region, because a live region injected into the document at
 * the same moment as its message is frequently not announced at all: the region
 * has to already be there for a screen reader to be watching it.
 */
export function announce(message: string, urgency: Urgency = 'polite'): void {
  for (const listener of listeners) listener(message, urgency)
}

/** Subscribes the mounted regions. Returns a function that unsubscribes. */
export function subscribeToAnnouncements(listener: Listener): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
