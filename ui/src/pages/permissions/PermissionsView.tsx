import { useQuery } from '@tanstack/react-query'

import { permissionsQuery } from '@/api/queries'
import { useGuild } from '@/lib/guild-context'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { PermissionsForm } from '@/pages/permissions/PermissionsForm'

export function PermissionsView() {
  const { guild } = useGuild()
  const query = useQuery(permissionsQuery(guild.id))
  if (query.isPending) return <PageSkeleton stats={0} rows={12} />
  if (query.isError) return <QueryError error={query.error} retry={() => void query.refetch()} />
  // La clé remet le formulaire à zéro quand les données du serveur changent (après enregistrement).
  return <PermissionsForm key={JSON.stringify(query.data)} data={query.data} />
}
