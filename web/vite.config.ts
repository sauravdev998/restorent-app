import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
// From vitest, not vite: it is the same function widened to accept the `test`
// block below.
import { defineConfig } from 'vitest/config'

// The API in local development. Behind CloudFront in production the same
// origin serves both, so there is no CORS to configure and the session cookie
// simply works. Proxying here makes local development same origin too, so the
// cookie behaves exactly as it will in production.
const API_ORIGIN = 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: API_ORIGIN,
        changeOrigin: true,
        // Server sent events must not be buffered. Without this the kitchen
        // screen would receive nothing until the stream closed, which is to
        // say never.
        configure: (proxy) => {
          proxy.on('proxyRes', (proxyRes) => {
            if (proxyRes.headers['content-type']?.includes('text/event-stream')) {
              proxyRes.headers['cache-control'] = 'no-cache, no-transform'
            }
          })
        },
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    css: true,
  },
})
