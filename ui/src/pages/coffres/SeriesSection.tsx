import { useQuery } from '@tanstack/react-query'

import { coffreSeriesQuery } from '@/api/queries'
import { Card, CardContent } from '@/atoms/Card'
import { Skeleton } from '@/atoms/Skeleton'
import { cn } from '@/lib/utils'
import { EmptyState } from '@/molecules/EmptyState'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { CoffreSeriesChart } from '@/organisms/CoffreSeriesChart'

export function SeriesSection({
  guildId,
  objet,
  itemLabel,
  propriete,
  jours,
}: {
  guildId: string
  objet: string
  itemLabel: string
  propriete: string | undefined
  jours: string
}) {
  const series = useQuery(coffreSeriesQuery(guildId, objet, propriete, jours))
  return (
    <Section title={`Évolution : ${itemLabel}`} description="Stock cumulé et mouvements par jour sur la période.">
      {series.isPending ? (
        <Skeleton className="h-64 rounded-xl" />
      ) : series.isError ? (
        <QueryError error={series.error} retry={() => void series.refetch()} />
      ) : series.data.length === 0 ? (
        <EmptyState title="Aucune donnée sur la période" />
      ) : (
        <Card className={cn('pb-3', series.isPlaceholderData && 'opacity-60')}>
          <CardContent>
            <CoffreSeriesChart series={series.data} itemLabel={itemLabel} />
          </CardContent>
        </Card>
      )}
    </Section>
  )
}
