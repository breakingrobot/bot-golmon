import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { meQuery, ticketStaffQuery } from '@/api/queries'
import { PageContainer } from '@/components/layout'
import { Forbidden, PageHeader, PageSkeleton, QueryError } from '@/components/page'
import { TicketStatsBlock } from '@/components/ticket-stats'
import { Button } from '@/components/ui/button'

export const Route = createFileRoute('/tickets/staff')({ component: StaffPage })

function StaffPage() {
  const me = useQuery(meQuery)
  return (
    <PageContainer>
      {me.data && !me.data.tickets ? <Forbidden>Réservé au staff des tickets.</Forbidden> : <Staff />}
    </PageContainer>
  )
}

function Staff() {
  const query = useQuery(ticketStaffQuery)
  return (
    <>
      <Button asChild variant="link" className="mb-2 h-auto px-0">
        <Link to="/tickets">
          <ArrowLeft aria-hidden /> Tickets
        </Link>
      </Button>
      <PageHeader title="Statistiques du staff" description="Tickets des 30 derniers jours." />
      {query.isPending ? (
        <PageSkeleton stats={5} />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <TicketStatsBlock stats={query.data} />
      )}
    </>
  )
}
