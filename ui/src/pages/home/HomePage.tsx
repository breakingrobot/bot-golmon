import { useQuery } from '@tanstack/react-query'

import { meQuery } from '@/api/queries'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { HomeContent } from '@/pages/home/HomeContent'
import { PageContainer } from '@/templates/PageContainer'

export const HomePage = () => {
  const me = useQuery(meQuery)
  return (
    <PageContainer>
      {me.isPending ? (
        <PageSkeleton />
      ) : me.isError ? (
        <QueryError error={me.error} retry={() => void me.refetch()} />
      ) : (
        <HomeContent me={me.data} />
      )}
    </PageContainer>
  )
}
