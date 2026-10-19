import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { ticketStaffQuery } from '@/api/queries'
import { Button } from '@/atoms/Button'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { TicketStatsBlock } from '@/organisms/TicketStatsBlock'

export function StaffView() {
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
