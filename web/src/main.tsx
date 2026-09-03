import { QueryClientProvider } from '@tanstack/react-query'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router'

import { queryClient } from '@/app/query-client'
import { router } from '@/app/router'
import { handleSignedOut } from '@/shared/api/client'
import { signedOut } from '@/shared/session/signed-out'
import '@/shared/i18n'
import '@/styles/index.css'

/**
 * What a `401` on any request does, registered once for the whole app.
 *
 * It lives here rather than in the client because the answer is a navigation,
 * and the client has no router. Here both are in hand.
 */
handleSignedOut(() => {
  void router.navigate(signedOut(queryClient, window.location), { replace: true })
})

const container = document.getElementById('root')
if (!container) {
  throw new Error('index.html is missing the #root element')
}

createRoot(container).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
)
