import { useQuery } from '@tanstack/react-query'

import { overviewQuery } from '@/api/queries-site'
import { useGuild } from '@/lib/guild-context'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { OverviewView } from '@/pages/overview/OverviewView'

/** Accueil du serveur : ouvert à tout membre, chaque tuile suit son propre droit (vérifié par le serveur). */
export const OverviewPage = () => {
  const { guild } = useGuild()
  const query = useQuery(overviewQuery(guild.id))
  if (query.isPending) return <PageSkeleton stats={6} rows={0} />
  if (query.isError) return <QueryError error={query.error} retry={() => void query.refetch()} />
  return <OverviewView overview={query.data} />
}
