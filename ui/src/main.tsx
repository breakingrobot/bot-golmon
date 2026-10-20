import './styles.css'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { QueryClientConfig } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { ApiRequestError } from '@/api/client'
import { Toaster } from '@/atoms/Toaster'
import { createAppRouter } from '@/router'
import { ThemeSync } from '@/state/theme'

const queryConfig = {
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      // Pas de nouvel essai sur une réponse définitive du serveur (401, 403, 404, 422…).
      retry: (failureCount, error) =>
        !(error instanceof ApiRequestError && error.status >= 400 && error.status < 500) && failureCount < 2,
    },
  },
} satisfies QueryClientConfig

const queryClient = new QueryClient(queryConfig)

const router = createAppRouter(queryClient)

const container = document.querySelector('#app')
if (!container) throw new Error('Élément #app absent de index.html')

// Application installable : service worker (public/sw.js) en production seulement, jamais en maquette.
if (import.meta.env.PROD && import.meta.env.VITE_MOCK !== '1' && 'serviceWorker' in navigator) {
  globalThis.addEventListener('load', async () => {
    try {
      await navigator.serviceWorker.register('/sw.js')
    } catch {
      // Sans service worker, le site fonctionne normalement (pas d'installation ni de page hors ligne).
    }
  })
}

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
