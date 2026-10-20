import { canSeePage } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { AbsencesView } from '@/pages/absences/AbsencesView'

export const AbsencesPage = () => {
  const { guild } = useGuild()
  if (!canSeePage(guild, 'absences')) {
    return <Forbidden>Les absences sont réservées aux membres du groupe (rôle membre, gradé ou lead).</Forbidden>
  }
  return <AbsencesView />
}
