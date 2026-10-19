import { useGuild } from '@/lib/guild-context'
import { Forbidden } from '@/molecules/Forbidden'
import { PermissionsView } from '@/pages/permissions/PermissionsView'

export function PermissionsPage() {
  const { guild } = useGuild()
  if (!guild.isAdmin) return <Forbidden>Réservé aux personnes qui peuvent « Gérer le serveur ».</Forbidden>
  return <PermissionsView />
}
