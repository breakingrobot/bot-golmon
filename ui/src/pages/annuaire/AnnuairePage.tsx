import { canSeePage } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { AnnuaireView } from '@/pages/annuaire/AnnuaireView'

export const AnnuairePage = () => {
  const { guild } = useGuild()
  if (!canSeePage(guild, 'annuaire')) return <Forbidden />
  return <AnnuaireView />
}
