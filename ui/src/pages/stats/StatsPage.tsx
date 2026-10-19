import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { StatsView } from '@/pages/stats/StatsView'

export function StatsPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'site.stats')) return <Forbidden />
  return <StatsView />
}
