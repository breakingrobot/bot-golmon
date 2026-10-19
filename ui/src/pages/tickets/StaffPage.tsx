import { useQuery } from '@tanstack/react-query'

import { meQuery } from '@/api/queries'
import { Forbidden } from '@/molecules/Forbidden'
import { StaffView } from '@/pages/tickets/StaffView'
import { PageContainer } from '@/templates/PageContainer'

export function StaffPage() {
  const me = useQuery(meQuery)
  return (
    <PageContainer>
      {me.data && !me.data.tickets ? <Forbidden>Réservé au staff des tickets.</Forbidden> : <StaffView />}
    </PageContainer>
  )
}
