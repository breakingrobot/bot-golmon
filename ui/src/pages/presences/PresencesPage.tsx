import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { PresencesView } from '@/pages/presences/PresencesView'

export const PresencesPage = () => {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'presence.stats')) return <Forbidden />
  return <PresencesView />
}
