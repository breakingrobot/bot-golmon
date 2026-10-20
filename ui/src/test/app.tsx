// Application complète (routeur en mémoire, client neuf) pour les tests de pages servies par la maquette.

import { QueryClientProvider } from '@tanstack/react-query'
import { createMemoryHistory, RouterProvider } from '@tanstack/react-router'
import { render } from '@testing-library/react'

import { createAppRouter } from '@/router'
import { createTestQueryClient } from '@/test/render'

export const MOCK_GUILD = '1187000000000000042'

export const renderApp = (path: string) => {
  const router = createAppRouter(createTestQueryClient(), createMemoryHistory({ initialEntries: [path] }))
  render(
    <QueryClientProvider client={router.options.context.queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )
  return router
}
