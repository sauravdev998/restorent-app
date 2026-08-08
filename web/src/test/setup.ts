import '@testing-library/jest-dom/vitest'

import { cleanup } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

// jsdom has no EventSource, so anything holding the live stream open would
// throw. A minimal stand in keeps component tests honest without pretending to
// test the stream itself.
class FakeEventSource {
  onopen: (() => void) | null = null
  onerror: (() => void) | null = null
  addEventListener = vi.fn()
  removeEventListener = vi.fn()
  close = vi.fn()
}

vi.stubGlobal('EventSource', FakeEventSource)

afterEach(() => {
  cleanup()
})
