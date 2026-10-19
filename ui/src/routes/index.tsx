import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { ArrowRight, LogIn, ShieldOff, Ticket } from 'lucide-react'

import { meQuery } from '@/api/queries'
import type { GuildAccess, Me } from '@/api/types'
import { GuildPageLink } from '@/components/guild-page-link'
import { PageContainer } from '@/components/layout'
import { LoginLink } from '@/components/login-link'
import { EmptyState, PageSkeleton, QueryError } from '@/components/page'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { visiblePages } from '@/lib/guild'
import { initials } from '@/lib/utils'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  const me = useQuery(meQuery)
  return (
    <PageContainer>
      {me.isPending ? (
        <PageSkeleton />
      ) : me.isError ? (
        <QueryError error={me.error} retry={() => void me.refetch()} />
      ) : (
        <HomeContent me={me.data} />
      )}
    </PageContainer>
  )
}

function HomeContent({ me }: { me: Me }) {
  if (!me.user) return <LoginCard me={me} />
  if (!me.tickets && me.guilds.length === 0) {
    return (
      <>
        <h1 className="text-2xl font-semibold tracking-tight">Bienvenue, {me.user.name}</h1>
        <EmptyState title="Aucune page accessible" icon={<ShieldOff />} className="mt-6">
          <p>
            Ce compte n'a accès à aucune page : il faut faire partie du staff des tickets, ou avoir sur un serveur du
            bot un rôle qui ouvre une page du site (<code className="font-mono text-xs">/config permission</code> sur
            Discord).
          </p>
        </EmptyState>
      </>
    )
  }
  return (
    <>
      <h1 className="text-2xl font-semibold tracking-tight">Bienvenue, {me.user.name}</h1>
      <p className="mt-1 text-sm text-ink-muted">Les pages ouvertes à votre compte, par serveur.</p>
      <div className="mt-6 grid gap-4 md:grid-cols-2">
        {me.guilds.map((guild) => (
          <GuildTile key={guild.id} guild={guild} userId={me.user?.id ?? ''} />
        ))}
        {me.tickets && (
          <Card className="transition-colors hover:border-line-strong">
            <CardHeader>
              <div className="flex items-center gap-3">
                <span className="flex size-10 items-center justify-center rounded-lg bg-user-soft text-user">
                  <Ticket className="size-5" aria-hidden />
                </span>
                <div>
                  <CardTitle>Tickets</CardTitle>
                  <CardDescription>Recherche, logs et statistiques du staff</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className="flex flex-wrap gap-2">
              <Button asChild size="sm">
                <Link to="/tickets">
                  Tickets <ArrowRight aria-hidden />
                </Link>
              </Button>
              <Button asChild size="sm" variant="outline">
                <Link to="/tickets/staff">Statistiques du staff</Link>
              </Button>
            </CardContent>
          </Card>
        )}
      </div>
    </>
  )
}

function GuildIcon({ guild }: { guild: GuildAccess }) {
  return guild.iconUrl ? (
    <img src={guild.iconUrl} alt="" className="size-10 rounded-lg" />
  ) : (
    <span
      className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-brand-soft text-sm font-semibold text-brand"
      aria-hidden
    >
      {initials(guild.name)}
    </span>
  )
}

function GuildTile({ guild, userId }: { guild: GuildAccess; userId: string }) {
  const pages = visiblePages(guild)
  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <GuildIcon guild={guild} />
          <div className="min-w-0">
            <CardTitle className="truncate">{guild.name}</CardTitle>
            <CardDescription>{guild.isAdmin ? 'Administration du serveur' : 'Membre'}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <ul className="grid gap-1 sm:grid-cols-2">
          {pages.map((page) => (
            <li key={page.key}>
              <GuildPageLink
                page={page}
                guildId={guild.id}
                userId={userId}
                className="group flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm hover:bg-muted"
              >
                <page.icon className="size-4 text-ink-subtle group-hover:text-brand" aria-hidden />
                <span className="font-medium text-ink">{page.label}</span>
              </GuildPageLink>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}

function LoginCard({ me }: { me: Me }) {
  return (
    <div className="mx-auto mt-8 max-w-md">
      <Card>
        <CardHeader>
          <CardTitle className="text-xl">Connexion</CardTitle>
          <CardDescription>
            {me.loginEnabled
              ? 'Connectez-vous avec Discord pour consulter les tickets et les pages de vos serveurs.'
              : "La connexion Discord n'est pas configurée : seuls les logs de tickets sont accessibles, par leur lien."}
          </CardDescription>
        </CardHeader>
        {me.loginEnabled && (
          <CardContent className="space-y-3">
            <Button asChild className="w-full" size="lg">
              <LoginLink loginUrl={me.loginUrl}>
                <LogIn aria-hidden /> Se connecter avec Discord
              </LoginLink>
            </Button>
            <p className="text-xs text-ink-subtle">
              Discord ne partage que votre pseudo et votre avatar : le site ne lit ni vos serveurs ni vos messages.
            </p>
          </CardContent>
        )}
      </Card>
    </div>
  )
}
