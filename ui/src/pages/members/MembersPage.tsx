import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { MembersView } from '@/pages/members/MembersView'

export const MembersPage = () => {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'membre.fiche')) return <Forbidden />
  return <MembersView />
}
