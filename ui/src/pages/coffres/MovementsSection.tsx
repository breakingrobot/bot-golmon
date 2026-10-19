import { useQuery } from '@tanstack/react-query'

import { type CoffreFilters, coffreMovementsQuery } from '@/api/queries'
import { Card } from '@/atoms/Card'
import { formatNumber } from '@/lib/format'
import { cn } from '@/lib/utils'
import { EmptyState } from '@/molecules/EmptyState'
import { Pagination } from '@/molecules/Pagination'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { MovementsTable } from '@/organisms/MovementsTable'

export function MovementsSection({
  guildId,
  filters,
  page,
  onPage,
}: {
  guildId: string
  filters: CoffreFilters
  page: number
  onPage: (page: number) => void
}) {
  const movements = useQuery(coffreMovementsQuery(guildId, filters, page))
  return (
    <Section
      title="Mouvements"
      description={
        movements.data
          ? `${formatNumber(movements.data.total)} mouvement(s), du plus récent au plus ancien.`
          : undefined
      }
    >
      {movements.isPending ? (
        <TableSkeleton />
      ) : movements.isError ? (
        <QueryError error={movements.error} retry={() => void movements.refetch()} />
      ) : movements.data.items.length === 0 ? (
        <EmptyState title="Aucun mouvement sur la sélection" />
      ) : (
        <>
          <Card className={cn('py-0', movements.isPlaceholderData && 'opacity-60')}>
            <MovementsTable items={movements.data.items} guildId={guildId} />
          </Card>
          <Pagination
            page={movements.data.page}
            pageSize={movements.data.pageSize}
            total={movements.data.total}
            onPage={onPage}
            newerLabel="Plus récents"
            olderLabel="Plus anciens"
          />
        </>
      )}
    </Section>
  )
}
