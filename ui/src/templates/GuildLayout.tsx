import { useQuery } from '@tanstack/react-query'
import { getRouteApi } from '@tanstack/react-router'

import { meQuery } from '@/api/queries'
import { Button } from '@/atoms/Button'
import { LoginLink } from '@/atoms/LoginLink'
import { findGuild } from '@/lib/guild'
import { EmptyState } from '@/molecules/EmptyState'
import { Forbidden } from '@/molecules/Forbidden'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { GuildShell } from '@/templates/GuildShell'
import { PageContainer } from '@/templates/PageContainer'

const route = getRouteApi('/serveurs/$guildId')

export function GuildLayout() {
  const { guildId } = route.useParams()
  const me = useQuery(meQuery)

  if (me.isPending) {
    return (
      <PageContainer wide>
        <PageSkeleton />
      </PageContainer>
    )
  }
  const data = me.data
  if (!data?.user) {
    return (
      <PageContainer>
        <EmptyState title="Connexion requise" className="mt-4">
          <p>Connectez-vous avec Discord pour accéder aux pages de ce serveur.</p>
          {data?.loginEnabled && (
            <Button asChild className="mt-4">
              <LoginLink loginUrl={data.loginUrl}>Se connecter</LoginLink>
            </Button>
          )}
        </EmptyState>
      </PageContainer>
    )
  }
  const guild = findGuild(data, guildId)
  if (!guild) {
    return (
      <PageContainer>
        <Forbidden>
          <p>Ce serveur n'existe pas, le bot n'y est pas, ou votre compte n'y a accès à aucune page.</p>
        </Forbidden>
      </PageContainer>
    )
  }
  return <GuildShell me={data} guild={guild} userId={data.user.id} />
}
