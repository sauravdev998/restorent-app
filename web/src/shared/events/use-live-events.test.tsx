import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { useLiveEvents } from './use-live-events'

/**
 * A stand in for the browser's EventSource that lets a test drive `readyState`.
 *
 * The distinction it exists to model: the browser retries a dropped connection
 * on its own and leaves `readyState` at CONNECTING, but treats a response it
 * cannot use (a 401, for example) as fatal, closing the stream for good and
 * leaving `readyState` at CLOSED. Both arrive through the same `onerror`.
 */
class ControllableEventSource {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSED = 2

  /** Every instance made during a test, so a test can drive the live one. */
  static instances: ControllableEventSource[] = []

  readyState: number = ControllableEventSource.CONNECTING
  onopen: (() => void) | null = null
  onerror: (() => void) | null = null

  /** Handlers registered per event name, so a test can send one down. */
  private readonly listeners = new Map<string, ((event: MessageEvent<string>) => void)[]>()

  addEventListener = vi.fn((name: string, handler: (event: MessageEvent<string>) => void) => {
    const existing = this.listeners.get(name) ?? []
    existing.push(handler)
    this.listeners.set(name, existing)
  })

  removeEventListener = vi.fn()
  close = vi.fn(() => {
    this.readyState = ControllableEventSource.CLOSED
  })

  /** Where the hook pointed this stream. */
  readonly url: string

  constructor(url: string) {
    this.url = url
    ControllableEventSource.instances.push(this)
  }

  /** The server sent a named message down the open stream. */
  emit(name: string, data: string) {
    for (const handler of this.listeners.get(name) ?? []) {
      handler(new MessageEvent(name, { data }))
    }
  }

  /** The browser accepted the response and the stream is live. */
  open() {
    this.readyState = ControllableEventSource.OPEN
    this.onopen?.()
  }

  /** The connection dropped mid stream. The browser will retry by itself. */
  dropMidStream() {
    this.readyState = ControllableEventSource.CONNECTING
    this.onerror?.()
  }

  /** The response was unusable. The browser gives up and never retries. */
  failForGood() {
    this.readyState = ControllableEventSource.CLOSED
    this.onerror?.()
  }
}

function wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })

  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
}

describe('useLiveEvents', () => {
  beforeEach(() => {
    ControllableEventSource.instances = []
    vi.stubGlobal('EventSource', ControllableEventSource)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  function live() {
    // The stream reports a fatal failure through this rather than navigating
    // itself, so the tests can watch for it and the hook stays free of a router.
    const onFatal = vi.fn()
    const rendered = renderHook(() => useLiveEvents(onFatal), { wrapper })
    const source = ControllableEventSource.instances.at(-1)
    if (!source) {
      throw new Error('the hook did not open a stream')
    }

    return { rendered, source, onFatal }
  }

  /**
   * The same, but holding on to the query client, for the tests about what the
   * hook does to the cache.
   */
  function liveWithClient() {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })

    const rendered = renderHook(() => useLiveEvents(() => undefined), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      ),
    })

    const source = ControllableEventSource.instances.at(-1)
    if (!source) {
      throw new Error('the hook did not open a stream')
    }

    return { rendered, source, queryClient }
  }

  function changeOf(entity: string, entityId: string) {
    return JSON.stringify({ entity, entity_id: entityId })
  }

  it('reports the stream as open once the browser accepts it', () => {
    const { rendered, source } = live()
    expect(rendered.result.current.status).toBe('connecting')

    act(() => {
      source.open()
    })
    expect(rendered.result.current.status).toBe('open')
  })

  it('reports a connection that dropped mid stream as connecting, because the browser retries', () => {
    const { rendered, source } = live()

    act(() => {
      source.open()
    })
    act(() => {
      source.dropMidStream()
    })

    expect(rendered.result.current.status).toBe('connecting')
  })

  // The bug this pair covers, found by driving the real app: killing the API
  // left the kitchen screen reading "Connecting" and showing no warning band
  // for as long as anybody watched. The browser retries a dead server forever
  // and never reaches the fatal state, so treating only the fatal state as down
  // meant the one outage that matters on a shift was never named. A screen that
  // is receiving nothing has to say so.
  it('calls a stream that has been reconnecting too long closed, so the screens can warn', () => {
    vi.useFakeTimers()

    try {
      const { rendered, source } = live()

      act(() => {
        source.open()
      })
      act(() => {
        source.dropMidStream()
      })
      expect(rendered.result.current.status).toBe('connecting')

      act(() => {
        vi.advanceTimersByTime(5_000)
      })

      expect(rendered.result.current.status).toBe('closed')
    } finally {
      vi.useRealTimers()
    }
  }) // covers: AC-16

  // The half that makes the other half work. Every failed retry arrives through
  // `onerror`, and they arrive closer together than the grace period. Writing
  // "connecting" on each one would restart the wait every time and the warning
  // would never appear, which is exactly the shape of the original bug.
  it('stays closed while the browser keeps retrying and failing', () => {
    vi.useFakeTimers()

    try {
      const { rendered, source } = live()

      act(() => {
        source.open()
      })
      act(() => {
        source.dropMidStream()
      })
      act(() => {
        vi.advanceTimersByTime(5_000)
      })
      expect(rendered.result.current.status).toBe('closed')

      // Three more failed attempts, at the cadence a browser actually uses.
      for (let attempt = 0; attempt < 3; attempt += 1) {
        act(() => {
          vi.advanceTimersByTime(3_000)
        })
        act(() => {
          source.dropMidStream()
        })
      }

      expect(rendered.result.current.status).toBe('closed')
    } finally {
      vi.useRealTimers()
    }
  }) // covers: AC-16

  // A blip must not flash a band across a kitchen screen, and the recovery has
  // to clear the warning without a reload.
  it('rides out a blip in silence and clears once the stream opens again', () => {
    vi.useFakeTimers()

    try {
      const { rendered, source } = live()

      act(() => {
        source.open()
      })
      act(() => {
        source.dropMidStream()
      })
      act(() => {
        vi.advanceTimersByTime(2_000)
      })
      expect(rendered.result.current.status).toBe('connecting')

      act(() => {
        source.open()
      })
      expect(rendered.result.current.status).toBe('open')

      // And the wait that was in flight must not fire behind the recovery.
      act(() => {
        vi.advanceTimersByTime(10_000)
      })
      expect(rendered.result.current.status).toBe('open')
    } finally {
      vi.useRealTimers()
    }
  }) // covers: AC-16

  // Being down is not being signed out. A retryable outage must never take the
  // session ending path, or a kitchen wifi blip would sign the chef out.
  it('never reports a retryable outage as the session ending', () => {
    vi.useFakeTimers()

    try {
      const { rendered, source, onFatal } = live()

      act(() => {
        source.open()
      })
      act(() => {
        source.dropMidStream()
      })
      act(() => {
        vi.advanceTimersByTime(30_000)
      })

      expect(rendered.result.current.status).toBe('closed')
      expect(onFatal).not.toHaveBeenCalled()
    } finally {
      vi.useRealTimers()
    }
  }) // covers: AC-16

  // The regression this file exists for. `onerror` used to set `connecting`
  // whatever had happened, so a stream the browser had abandoned for good still
  // read as "Connecting" forever. That tells staff to wait for a reconnect that
  // is never coming, when the only thing that would help is a reload.
  it('reports a stream the browser has abandoned as closed, not connecting', () => {
    const { rendered, source } = live()

    act(() => {
      source.failForGood()
    })

    expect(rendered.result.current.status).toBe('closed')
    expect(rendered.result.current.status).not.toBe('connecting')
  })

  // Rule 1 in the hook's own doc comment, and the reason the server writes a
  // comment the instant a stream is subscribed. Postgres queues nothing for a
  // listener that is not connected, so anything published during a gap is gone.
  // Refetching only on the first open would turn one blip into a ticket that
  // never appears in the kitchen, with nothing anywhere reporting an error.
  it('refetches every active query each time the stream opens, not only the first time', () => {
    const { source, queryClient } = liveWithClient()
    const refetchQueries = vi.spyOn(queryClient, 'refetchQueries')

    act(() => {
      source.open()
    })

    expect(refetchQueries).toHaveBeenCalledWith({ type: 'active' })
    expect(refetchQueries).toHaveBeenCalledTimes(1)

    // A blip, then the browser reconnects by itself.
    act(() => {
      source.dropMidStream()
    })
    act(() => {
      source.open()
    })

    expect(refetchQueries).toHaveBeenCalledTimes(2)
  })

  // Rule 2. The stream must not become a second way to read data: it carries a
  // kind and an id, the client goes back and asks for the row, and row level
  // security decides whether it may have it. Writing the payload into the cache
  // would hand a screen content that nothing checked it was allowed to see.
  it('invalidates what an event touched and never writes the payload into the cache', () => {
    const { source, queryClient } = liveWithClient()
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')
    const setQueryData = vi.spyOn(queryClient, 'setQueryData')

    act(() => {
      source.open()
    })
    act(() => {
      source.emit('entity_changed', changeOf('order_line', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'))
    })

    // Exactly the two prefixes an order line feeds, from the written map.
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['order_round'] })
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['visit'] })
    expect(setQueryData).not.toHaveBeenCalled()
  }) // covers: AC-15

  // The narrowing is the point of the map, so this is the assertion that would
  // fail if somebody widened it back to "refetch everything". A chef marking one
  // dish must not send the browser back for the menu or the floor.
  it('invalidates only the keys its own entity kind feeds', () => {
    const { source, queryClient } = liveWithClient()
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')

    act(() => {
      source.open()
    })
    act(() => {
      source.emit('entity_changed', changeOf('order_line', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'))
    })

    const invalidated = invalidateQueries.mock.calls.map(([call]) => JSON.stringify(call?.queryKey))

    expect(invalidated).toHaveLength(2)
    expect(invalidated).not.toContain(JSON.stringify(['dish']))
    expect(invalidated).not.toContain(JSON.stringify(['visit', 'floor']))
  }) // covers: AC-15

  // `probe` carries no product meaning: it exists so the development endpoint
  // can prove the whole path with nothing behind it. Invalidating on it would
  // send every open screen back to the API for a message about nothing.
  it('invalidates nothing at all for a probe', () => {
    const { source, queryClient } = liveWithClient()
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')

    act(() => {
      source.open()
    })
    act(() => {
      source.emit('entity_changed', changeOf('probe', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'))
    })

    expect(invalidateQueries).not.toHaveBeenCalled()
  }) // covers: AC-15

  // An entity string this build has never heard of. Dropped and logged rather
  // than guessed: the refetch on the next stream open catches up whatever it
  // was about, and acting on the wrong entity is a bug a user sees.
  it('drops an event for an entity kind it does not know', () => {
    const noise = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const { source, queryClient } = liveWithClient()
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')

    try {
      act(() => {
        source.open()
      })
      act(() => {
        source.emit(
          'entity_changed',
          changeOf('table_section', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'),
        )
      })

      expect(invalidateQueries).not.toHaveBeenCalled()
      expect(noise).toHaveBeenCalledOnce()
    } finally {
      noise.mockRestore()
    }
  }) // covers: AC-15

  it('counts an event and remembers the last one', () => {
    const { rendered, source } = live()

    act(() => {
      source.open()
    })
    expect(rendered.result.current.received).toBe(0)
    expect(rendered.result.current.last).toBeNull()

    act(() => {
      source.emit('entity_changed', changeOf('probe', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'))
    })

    expect(rendered.result.current.received).toBe(1)
    expect(rendered.result.current.last).toEqual({
      entity: 'probe',
      entity_id: 'f7c4899b-0e46-4f51-a1b8-827357a2b06f',
    })
  })

  it('ignores a payload it cannot read and keeps delivering the ones it can', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { rendered, source } = live()

    act(() => {
      source.open()
    })
    act(() => {
      source.emit('entity_changed', 'this is not json')
    })

    expect(rendered.result.current.received).toBe(0)
    expect(rendered.result.current.last).toBeNull()
    expect(consoleError).toHaveBeenCalled()

    // The stream is still usable. One bad payload must not cost this screen
    // every event that comes after it.
    act(() => {
      source.emit('entity_changed', changeOf('probe', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'))
    })

    expect(rendered.result.current.received).toBe(1)
  })

  it('resynchronises when the server says the screen fell too far behind', () => {
    const { source, queryClient } = liveWithClient()

    act(() => {
      source.open()
    })

    const refetchQueries = vi.spyOn(queryClient, 'refetchQueries')
    act(() => {
      source.emit('resync', '')
    })

    expect(refetchQueries).toHaveBeenCalledWith({ type: 'active' })
  })

  it('names no restaurant in the address, because the cookie already does', () => {
    // The regression this guards is the placeholder coming back. `EventSource`
    // cannot set a header, which is why the development placeholder put the
    // restaurant in the query string; now the session cookie says which
    // restaurant this is, and a browser that could name one would be a way to
    // ask for somebody else's stream.
    const { source } = live()

    expect(source.url).toBe('/api/events')
  }) // covers: AC-9

  it('reports a fatal stream failure as being signed out', () => {
    // A fatal error on this stream is almost always a 401: the session was
    // revoked or expired and the browser will not retry a response it cannot
    // use. It is the same event an ordinary request's 401 is, so it takes the
    // same path rather than a second one that could drift.
    const { source, onFatal } = live()

    act(() => {
      source.failForGood()
    })

    expect(onFatal).toHaveBeenCalledTimes(1)
  }) // covers: AC-18, AC-19

  it('does not report a dropped connection as being signed out', () => {
    // The browser retries this one on its own. Sending somebody to the sign in
    // screen because a phone walked behind a wall is the failure this
    // distinction exists to prevent.
    const { source, onFatal } = live()

    act(() => {
      source.dropMidStream()
    })

    expect(onFatal).not.toHaveBeenCalled()
  }) // covers: AC-19

  it('closes the stream when the screen goes away', () => {
    const { rendered, source } = live()

    act(() => {
      source.open()
    })
    rendered.unmount()

    expect(source.close).toHaveBeenCalled()
  })
})
