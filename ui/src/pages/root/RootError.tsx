import { useRouter } from '@tanstack/react-router'
import type { ErrorComponentProps } from '@tanstack/react-router'
import { CircleAlert } from 'lucide-react'

import { errorMessage } from '@/api/client'
import { Button } from '@/atoms/Button'
import { SiteHeader } from '@/organisms/SiteHeader'
import { PageContainer } from '@/templates/PageContainer'

export const RootError = ({ error, reset }: ErrorComponentProps) => {
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
