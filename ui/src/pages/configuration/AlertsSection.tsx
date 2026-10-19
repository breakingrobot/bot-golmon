import { useQuery } from '@tanstack/react-query'

import { coffreAlertsQuery, coffreMetaQuery } from '@/api/queries'
import { Skeleton } from '@/atoms/Skeleton'
import { useGuild } from '@/lib/guild-context'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { AlertsForm } from '@/pages/configuration/AlertsForm'

export const AlertsSection = () => {
  const { guild } = useGuild()
  const alerts = useQuery(coffreAlertsQuery(guild.id))
  const meta = useQuery(coffreMetaQuery(guild.id))
  return (
    <Section
      title="Alertes des coffres"
      description="Messages publiés sur Discord quand un stock passe sous un seuil ou qu'un retrait sort de l'ordinaire."
    >
      {alerts.isPending || meta.isPending ? (
        <Skeleton className="h-64 rounded-xl" />
      ) : alerts.isError ? (
        <QueryError error={alerts.error} retry={() => void alerts.refetch()} />
      ) : meta.isError ? (
        <QueryError error={meta.error} retry={() => void meta.refetch()} />
      ) : (
        <AlertsForm key={JSON.stringify(alerts.data)} initial={alerts.data} meta={meta.data} />
      )}
    </Section>
  )
}
