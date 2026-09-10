import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
// From vitest, not vite: it is the same function widened to accept the `test`
// block below.
import { defineConfig } from 'vitest/config'
import type { ProxyOptions } from 'vite'

// The API in local development. Behind CloudFront in production the same
// origin serves both, so there is no CORS to configure and the session cookie
// simply works. Proxying here makes local development same origin too, so the
// cookie behaves exactly as it will in production.
const API_ORIGIN = 'http://127.0.0.1:8080'

/**
 * The `/api` proxy, shared by the dev server and the preview server.
 *
 * `preview` needs it too, and that is not an optimisation. The browser tests
 * run against the production build served by `vite preview`, because the dev
 * server's hot reload client holds a connection of its own and reconnects on
 * its own schedule, which is noise in a test whose whole subject is a
 * connection staying open. Without the proxy here, `preview` would serve the
 * app and answer every `/api` call with its own 404, and the session cookie
 * would stop being same origin.
 */
const apiProxy: Record<string, ProxyOptions> = {
  '/api': {
    target: API_ORIGIN,
    changeOrigin: true,
    // Server sent events must not be buffered. Without this the kitchen
    // screen would receive nothing until the stream closed, which is to
    // say never.
    configure: (proxy) => {
      proxy.on('proxyRes', (proxyRes, _req, res) => {
        if (!proxyRes.headers['content-type']?.includes('text/event-stream')) {
          return
        }

        proxyRes.headers['cache-control'] = 'no-cache, no-transform'

        // Hand the upstream's death down to the browser.
        //
        // The proxy does not do this by itself: when the API process dies
        // it drops its socket, and the proxy simply stops forwarding while
        // leaving the browser's own connection open. The browser is waiting
        // on a stream nobody is writing to any more, so it never sees an
        // error, never reconnects, and shows a screen that looks live while
        // receiving nothing. Measured: an API killed mid stream closed a
        // direct client instantly and left a proxied one hanging past 25
        // seconds.
        //
        // Destroying the client response makes local development behave
        // like production, where CloudFront and the load balancer pass the
        // close along and the browser reconnects on its own.
        proxyRes.on('close', () => {
          res.destroy()
        })
      })

      // Same reasoning for a stream that fails rather than closing: leave
      // the browser hanging and it never learns to retry.
      //
      // Scoped to event streams only, and the scoping is the point. Vite
      // registers its own error handler after this one (it calls
      // `configure` first), and handlers run in registration order, so
      // destroying the socket here happens before Vite can answer. Vite
      // still writes its `502 http proxy error` and still logs, but the
      // browser never receives it: it gets a bare connection reset instead.
      // Unscoped, that traded Vite's diagnosable 502 for `Empty reply from
      // server` on every ordinary `/api/*` call made while the API is down,
      // which is most of local development.
      //
      // The response has no content-type to read here, because there is no
      // response. The request's `Accept` header is the signal instead:
      // `EventSource` is specified to send `text/event-stream`.
      proxy.on('error', (_error, req, res) => {
        if (!req.headers.accept?.includes('text/event-stream')) {
          return
        }

        if ('destroy' in res) {
          res.destroy()
        }
      })
    },
  },
}

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      // The language catalogue is shared with the API, which compiles the same
      // file in, so it lives at the repository root rather than inside either
      // side. The alias is what keeps that out of every import as a chain of
      // ../.. and what keeps it importable from a package it sits outside.
      '@catalogue': fileURLToPath(new URL('../locales/catalogue.json', import.meta.url)),
    },
  },
  server: { proxy: apiProxy },
  preview: { proxy: apiProxy },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    css: true,
    // Vitest and Playwright both claim `*.spec.ts`, and the browser scenario
    // needs a browser, a running API, and a database. Left in, Vitest picks it
    // up, fails to parse `@playwright/test` in jsdom, and reports a failure
    // that has nothing to do with any unit under test.
    exclude: ['e2e/**', 'node_modules/**', 'dist/**'],
  },
})
