import { canSeePage } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { RecruesView } from '@/pages/recrues/RecruesView'

export const RecruesPage = () => {
  const { guild } = useGuild()
  if (!canSeePage(guild, 'recrues')) {
    return (
      <Forbidden>
        Le suivi des recrues est réservé aux rôles gradé et lead (actions strictes : « Gérer le serveur » n'y donne pas
        accès).
      </Forbidden>
    )
  }
  return <RecruesView />
}
