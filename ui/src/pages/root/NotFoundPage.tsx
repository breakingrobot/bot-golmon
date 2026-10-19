import { NotFoundState } from '@/molecules/NotFoundState'
import { PageContainer } from '@/templates/PageContainer'

/** Adresse inconnue (route racine `notFoundComponent`). */
export function NotFoundPage() {
  return (
    <PageContainer>
      <NotFoundState />
    </PageContainer>
  )
}
