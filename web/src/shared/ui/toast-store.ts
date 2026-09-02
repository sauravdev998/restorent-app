import { announce } from './announce'
import type { StatusTone } from './status'

export interface ToastRequest {
  title: string
  description?: string
  /** Decides the icon and the colour. Defaults to `ready`. */
  tone?: StatusTone
  /** How long before it disappears, in milliseconds. Defaults to six seconds. */
  durationMs?: number
}

export interface Toast extends ToastRequest {
  id: string
}

type Listener = (toasts: Toast[]) => void

const listeners = new Set<Listener>()
let toasts: Toast[] = []
let nextId = 0

function publish(): void {
  for (const listener of listeners) listener(toasts)
}

/**
 * Shows a transient confirmation, and says it out loud politely.
 *
 * Politely, not assertively: a toast confirms something the person just did, so
 * interrupting whatever they are reading to tell them it worked is rude. The
 * ready alert is the one that interrupts, and it is a different component.
 */
export function showToast(request: ToastRequest): string {
  const id = `toast-${String(nextId++)}`
  toasts = [...toasts, { ...request, id }]
  publish()

  announce(
    request.description === undefined ? request.title : `${request.title}. ${request.description}`,
    'polite',
  )

  return id
}

export function dismissToast(id: string): void {
  toasts = toasts.filter((toast) => toast.id !== id)
  publish()
}

export function subscribeToToasts(listener: Listener): () => void {
  listeners.add(listener)
  listener(toasts)
  return () => {
    listeners.delete(listener)
  }
}
