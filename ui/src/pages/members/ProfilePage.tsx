import { useQuery } from '@tanstack/react-query'
import { getRouteApi, Link } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { memberProfileQuery } from '@/api/queries'
import { Button } from '@/atoms/Button'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { ProfileView } from '@/pages/members/ProfileView'

const route = getRouteApi('/serveurs/$guildId/membres/$userId')

export const ProfilePage = () => {
  const { guild, userId: me } = useGuild()
  const { userId } = route.useParams()
  const query = useQuery(memberProfileQuery(guild.id, userId))
  const canList = hasPerm(guild, 'membre.fiche')

  return (
    <>
      {canList && (
        <Button asChild variant="link" className="mb-2 h-auto px-0">
          <Link to="/serveurs/$guildId/membres" params={{ guildId: guild.id }}>
            <ArrowLeft aria-hidden /> Membres
          </Link>
        </Button>
      )}
      {query.isPending ? (
        <PageSkeleton />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <ProfileView profile={query.data} own={userId === me} />
      )}
    </>
  )
}
