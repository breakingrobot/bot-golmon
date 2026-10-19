import './styles.css'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { ApiRequestError } from '@/api/client'
import { Toaster } from '@/components/ui/sonner'
import { ThemeProvider } from '@/lib/theme'
import { createAppRouter } from '@/router'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      // Pas de nouvel essai sur une réponse définitive du serveur (401, 403, 404, 422…).
      retry: (failureCount, error) =>
        !(error instanceof ApiRequestError && error.status >= 400 && error.status < 500) && failureCount < 2,
    },
  },
})

const router = createAppRouter(queryClient)

const container = document.getElementById('app')
if (!container) throw new Error('Élément #app absent de index.html')

createRoot(container).render(
  <StrictMode>
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
        <Toaster />
      </QueryClientProvider>
    </ThemeProvider>
  </StrictMode>,
)
