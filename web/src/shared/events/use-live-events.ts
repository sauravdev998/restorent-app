import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'

import { FAN_OUT, isEntityKind } from './query-keys'

/** One message from the server. A kind and an id, never row content. */
export interface StreamEvent {
  entity: string
  entity_id: string
}

export type StreamStatus = 'connecting' | 'open' | 'closed'

export interface LiveEvents {
  status: StreamStatus
  /** The most recent event, mainly useful for showing that the pipe is alive. */
  last: StreamEvent | null
  /** How many events have arrived since this browser loaded. */
  received: number
}

/**
 * Holds the live stream open and keeps the query cache honest.
 *
 * Two rules here are load bearing, and neither is obvious:
 *
 * 1. **Refetch every active query whenever the stream opens, including every
 *    reconnect.** Postgres queues nothing for a listener that is not connected,
 *    so anything published during a gap is simply gone. Without this refetch,
 *    one network blip becomes a ticket that never appears on the kitchen
 *    screen, and nothing anywhere reports an error.
 *
 * 2. **An event invalidates, it never writes.** The message carries a kind and
 *    an id, so the client goes back and asks for the row. That keeps row level
 *    security the single authority on who may see what. Trusting event contents
 *    would create a second, unguarded way to read data.
 *
 * What it invalidates is narrow and written down, in `query-keys.ts`. An event
 * refetches the query key prefixes its own entity kind actually feeds, so a
 * chef marking one dish does not send the whole browser back to the API for the
 * menu, the floor, and every open table.
 *
 * @param onFatal what to do when the stream fails in a way the browser will not
 * retry, which in practice means the session ended. It takes the same path an
 * ordinary request's `401` does.
 */
export function useLiveEvents(onFatal: () => void): LiveEvents {
  const queryClient = useQueryClient()
  const [status, setStatus] = useState<StreamStatus>('connecting')
  const [last, setLast] = useState<StreamEvent | null>(null)
  const [received, setReceived] = useState(0)

  useEffect(() => {
    // No restaurant in the address. `EventSource` cannot set a header, but it
    // does send cookies, and the session cookie is the only thing that says
    // which restaurant this stream belongs to.
    const source = new EventSource('/api/events', { withCredentials: true })

    const resynchronise = () => {
      void queryClient.refetchQueries({ type: 'active' })
    }

    source.onopen = () => {
      setStatus('open')
      // Rule 1. Every open, not just the first.
      resynchronise()
    }

    source.onerror = () => {
      // Two different failures arrive through this one callback, and only
      // `readyState` tells them apart.
      //
      // A connection that drops mid stream is retried by the browser on its
      // own, and `readyState` is CONNECTING while it does. But a response the
      // browser cannot use at all, a non 2xx status in particular, is fatal:
      // the spec has it close the stream and never retry, leaving `readyState`
      // at CLOSED. `/api/events` answers 401 whenever there is no session, so
      // this is the common case, not a corner one.
      //
      // Reporting "connecting" for a stream that is never coming back is worse
      // than reporting nothing, because it tells staff to wait when what they
      // need to do is reload.
      const fatal = source.readyState === EventSource.CLOSED
      setStatus(fatal ? 'closed' : 'connecting')

      // A fatal error on this stream is almost always a `401`: the session was
      // revoked, or expired, and the browser will not retry a response it
      // cannot use. It is the same event an ordinary request's `401` is, so it
      // takes the same path rather than a second one that could drift.
      if (fatal) onFatal()
    }

    source.addEventListener('entity_changed', (message: MessageEvent<string>) => {
      let event: StreamEvent
      try {
        event = JSON.parse(message.data) as StreamEvent
      } catch {
        console.error('ignoring an unreadable event payload', message.data)
        return
      }

      setLast(event)
      setReceived((count) => count + 1)

      // Rule 2. Invalidate exactly what this kind of change feeds, through the
      // written map, and let those queries refetch themselves.
      if (!isEntityKind(event.entity)) {
        // A kind this build has never heard of. Dropped rather than guessed:
        // invalidating on a string nobody wrote a row for would match nothing
        // anyway, and the refetch on the next stream open catches up whatever
        // it was about.
        console.error('ignoring an event for an unknown entity kind', event.entity)
        return
      }

      for (const queryKey of FAN_OUT[event.entity]) {
        void queryClient.invalidateQueries({ queryKey })
      }
    })

    source.addEventListener('resync', () => {
      // The server said this screen fell too far behind to be trusted.
      resynchronise()
    })

    return () => {
      source.close()
      setStatus('closed')
    }
  }, [queryClient, onFatal])

  return { status, last, received }
}
