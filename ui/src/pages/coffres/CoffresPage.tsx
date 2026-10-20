import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { CoffresView } from '@/pages/coffres/CoffresView'

export const CoffresPage = () => {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'coffre.voir')) return <Forbidden />
  return <CoffresView />
}
