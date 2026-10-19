import type { QueryClient } from '@tanstack/react-query'
import { createRootRouteWithContext } from '@tanstack/react-router'

import { meQuery } from '@/api/queries'
import { NotFoundPage } from '@/pages/root/NotFoundPage'
import { RootError } from '@/pages/root/RootError'
import { RootLayout } from '@/templates/RootLayout'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  beforeLoad: async ({ context }) => {
    await context.queryClient.ensureQueryData(meQuery)
  },
  component: RootLayout,
  notFoundComponent: NotFoundPage,
  errorComponent: RootError,
})
