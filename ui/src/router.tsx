import type { QueryClient } from '@tanstack/react-query'
import { createRouter } from '@tanstack/react-router'

import { PageSkeleton } from '@/components/page'
import { routeTree } from '@/routeTree.gen'

export function createAppRouter(queryClient: QueryClient) {
  return createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: 'intent',
    // Les données viennent de TanStack Query : le routeur ne garde pas de cache de loaders.
    defaultPreloadStaleTime: 0,
    defaultPendingComponent: () => <PageSkeleton />,
    scrollRestoration: true,
  })
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>
  }
}
