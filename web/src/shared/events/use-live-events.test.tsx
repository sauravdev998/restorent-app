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
    const rendered = renderHook(() => useLiveEvents(), { wrapper })
    const source = ControllableEventSource.instances.at(-1)
    if (!source) {
      throw new Error('the hook did not open a stream')
    }

    return { rendered, source }
  }

  /**
   * The same, but holding on to the query client, for the tests about what the
   * hook does to the cache.
   */
  function liveWithClient() {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })

    const rendered = renderHook(() => useLiveEvents(), {
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
      source.emit('entity_changed', changeOf('probe', 'f7c4899b-0e46-4f51-a1b8-827357a2b06f'))
    })

    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['probe'] })
    expect(setQueryData).not.toHaveBeenCalled()
  })

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

  it('opens the stream scoped to the restaurant this browser is acting for', () => {
    const { source } = live()

    expect(source.url).toContain('/api/events')
    expect(source.url).toContain('restaurant_id=')
  })

  it('closes the stream when the screen goes away', () => {
    const { rendered, source } = live()

    act(() => {
      source.open()
    })
    rendered.unmount()

    expect(source.close).toHaveBeenCalled()
  })
})
