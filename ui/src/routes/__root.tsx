import type { QueryClient } from '@tanstack/react-query'
import { Outlet, createRootRouteWithContext, useRouter, type ErrorComponentProps } from '@tanstack/react-router'
import { CircleAlert } from 'lucide-react'

import { errorMessage } from '@/api/client'
import { meQuery } from '@/api/queries'
import { PageContainer, SiteHeader } from '@/components/layout'
import { NotFoundState } from '@/components/page'
import { Button } from '@/components/ui/button'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  beforeLoad: async ({ context }) => {
    await context.queryClient.ensureQueryData(meQuery)
  },
  component: RootLayout,
  notFoundComponent: () => (
    <PageContainer>
      <NotFoundState />
    </PageContainer>
  ),
  errorComponent: RootError,
})

function RootLayout() {
  return (
    <div className="flex min-h-dvh flex-col">
      <a
        href="#contenu"
        className="sr-only z-50 rounded-md bg-card px-3 py-2 focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
      >
        Aller au contenu
      </a>
      <SiteHeader />
      <Outlet />
    </div>
  )
}

function RootError({ error, reset }: ErrorComponentProps) {
  const router = useRouter()
  return (
    <>
      <SiteHeader />
      <PageContainer>
        <div role="alert" className="rounded-xl border border-danger/40 bg-danger-soft p-6 text-danger">
          <div className="flex items-center gap-2 font-semibold">
            <CircleAlert className="size-5" aria-hidden /> Une erreur est survenue
          </div>
          <p className="mt-2 text-sm">{errorMessage(error)}</p>
          <Button
            variant="outline"
            size="sm"
            className="mt-4"
            onClick={() => {
              reset()
              void router.invalidate()
            }}
          >
            Réessayer
          </Button>
        </div>
      </PageContainer>
    </>
  )
}
