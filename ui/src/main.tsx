import './styles.css'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { ApiRequestError } from '@/api/client'
import { Toaster } from '@/components/ui/sonner'
import { createAppRouter } from '@/router'
import { ThemeSync } from '@/state/theme'

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
    {/* État d'interface : atomes Jotai dans le store par défaut, sans <Provider>. */}
    <ThemeSync />
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toaster />
    </QueryClientProvider>
  </StrictMode>,
)
