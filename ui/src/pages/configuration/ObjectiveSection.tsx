import { useQuery } from '@tanstack/react-query'

import { objectiveQuery } from '@/api/queries'
import { Skeleton } from '@/atoms/Skeleton'
import { useGuild } from '@/lib/guild-context'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { ObjectiveForm } from '@/pages/configuration/ObjectiveForm'

export function ObjectiveSection() {
  const { guild } = useGuild()
  const query = useQuery(objectiveQuery(guild.id))
  return (
    <Section
      title="Objectif de cotisation"
      description="Montant attendu de chaque membre par semaine (lundi → dimanche, heure de Paris)."
    >
      {query.isPending ? (
        <Skeleton className="h-40 rounded-xl" />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <ObjectiveForm key={JSON.stringify(query.data)} initial={query.data} />
      )}
    </Section>
  )
}
