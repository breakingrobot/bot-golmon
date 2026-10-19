import { useQuery } from '@tanstack/react-query'
import { getRouteApi } from '@tanstack/react-router'

import { ticketLogQuery } from '@/api/queries'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { LogView } from '@/pages/logs/LogView'
import { PageContainer } from '@/templates/PageContainer'

const route = getRouteApi('/logs/$threadId')

export const LogPage = () => {
  const { threadId } = route.useParams()
  const query = useQuery(ticketLogQuery(threadId))
  return (
    <PageContainer>
      <div className="mx-auto max-w-4xl">
        {query.isPending ? (
          <PageSkeleton stats={3} />
        ) : query.isError ? (
          <QueryError error={query.error} retry={() => void query.refetch()} />
        ) : (
          <LogView log={query.data} />
        )}
      </div>
    </PageContainer>
  )
}
