import { useQuery } from '@tanstack/react-query'

import { meQuery } from '@/api/queries'
import { Forbidden } from '@/molecules/Forbidden'
import { TicketsView } from '@/pages/tickets/TicketsView'
import { PageContainer } from '@/templates/PageContainer'

export const TicketsPage = () => {
  const me = useQuery(meQuery)
  return (
    <PageContainer>
      {me.data && !me.data.tickets ? <Forbidden>Réservé au staff des tickets.</Forbidden> : <TicketsView />}
    </PageContainer>
  )
}
