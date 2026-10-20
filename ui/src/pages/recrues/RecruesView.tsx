import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { UserPlus } from 'lucide-react'

import { recruesQuery } from '@/api/queries-groupe'
import { formatWeek } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import { EmptyState } from '@/molecules/EmptyState'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { RecrueCard } from '@/pages/recrues/RecrueCard'
import { RecrueHistorySheet } from '@/pages/recrues/RecrueHistorySheet'

const route = getRouteApi('/serveurs/$guildId/recrues')

export const RecruesView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/recrues' })
  const query = useQuery(recruesQuery(guild.id))
  const canVote = hasPerm(guild, 'recrue.avis')
  const canVeto = hasPerm(guild, 'recrue.veto')
  const canFollow = hasPerm(guild, 'recrue.suivi')
  const linkTo = hasPerm(guild, 'membre.fiche') ? guild.id : undefined
  const historyMember = query.data?.recruits.find((recrue) => recrue.member.id === search.recrue)?.member ?? null
  const setRecrue = (recrue: string | undefined) => void navigate({ search: compact({ recrue }), replace: true })

  return (
    <>
      <PageHeader
        title="Recrues"
        description={
          query.data?.weekStart
            ? `${guild.name} · ${formatWeek(query.data.weekStart)}`
            : `${guild.name} · avis de la semaine`
        }
      />
      {query.isPending ? (
        <PageSkeleton stats={0} />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : query.data.configured ? (
        query.data.recruits.length === 0 ? (
          <EmptyState title="Aucune recrue cette semaine" icon={<UserPlus />}>
            Les membres qui reçoivent le rôle recrue apparaissent ici, avec le décompte des avis.
          </EmptyState>
        ) : (
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            {query.data.recruits.map((recrue) => (
              <RecrueCard
                key={recrue.entryId}
                guildId={guild.id}
                recrue={recrue}
                canVote={canVote}
                canVeto={canVeto}
                canFollow={canFollow}
                linkTo={linkTo}
                onHistory={() => setRecrue(recrue.member.id)}
              />
            ))}
          </div>
        )
      ) : (
        <EmptyState title="Module non configuré" icon={<UserPlus />}>
          Un admin doit régler les rôles recrue et gradé sur Discord avec /config recrue.
        </EmptyState>
      )}
      {canFollow && (
        <RecrueHistorySheet guildId={guild.id} member={historyMember} onClose={() => setRecrue(undefined)} />
      )}
    </>
  )
}
