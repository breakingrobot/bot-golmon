import { NotFoundState } from '@/molecules/NotFoundState'
import { PageContainer } from '@/templates/PageContainer'

/** Adresse inconnue (route racine `notFoundComponent`). */
export const NotFoundPage = () => (
  <PageContainer>
    <NotFoundState />
  </PageContainer>
)
