import { QueryClientProvider } from '@tanstack/react-query'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router'

import { queryClient } from '@/app/query-client'
import { router } from '@/app/router'
import '@/shared/i18n'
import '@/styles/index.css'

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
