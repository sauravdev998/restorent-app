import '@testing-library/jest-dom/vitest'

import { cleanup } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

// jsdom has no EventSource, so anything holding the live stream open would
// throw. A minimal stand in keeps component tests honest without pretending to
// test the stream itself.
//
// It carries `readyState` and the three state constants because code that
// handles a stream error has to read them to tell a retry apart from a
// permanent failure. A stub without them reports `undefined` on both sides of
// that comparison, which quietly makes every error look alike and would let the
// bug this models slip through a passing test.
class FakeEventSource {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSED = 2

  readyState: number = FakeEventSource.CONNECTING
  onopen: (() => void) | null = null
  onerror: (() => void) | null = null
  addEventListener = vi.fn()
  removeEventListener = vi.fn()
  close = vi.fn(() => {
    this.readyState = FakeEventSource.CLOSED
  })
}

vi.stubGlobal('EventSource', FakeEventSource)

afterEach(() => {
  cleanup()
})
