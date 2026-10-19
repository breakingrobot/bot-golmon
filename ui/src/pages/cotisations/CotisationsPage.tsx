import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { CotisationsView } from '@/pages/cotisations/CotisationsView'

export function CotisationsPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'cotisation.stats')) return <Forbidden />
  return <CotisationsView />
}
