import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { HealthView } from '@/pages/health/HealthView'

export const HealthPage = () => {
  const { guild } = useGuild()
  if (!guild.isAdmin) return <Forbidden />
  return <HealthView />
}
