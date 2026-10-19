import { Link } from '@tanstack/react-router'
import { useAtomValue } from 'jotai'
import { ArrowRight, ShieldOff, Ticket } from 'lucide-react'

import type { Me } from '@/api/types'
import { Button } from '@/atoms/Button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/atoms/Card'
import { EmptyState } from '@/molecules/EmptyState'
import { GuildTile } from '@/pages/home/GuildTile'
import { LoginCard } from '@/pages/home/LoginCard'
import { lastGuildIdAtom } from '@/state/last-guild'

export type HomeContentProps = { me: Me }

export const HomeContent = ({ me }: HomeContentProps) => {
  const lastGuildId = useAtomValue(lastGuildIdAtom)
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
        {/* Dernier serveur ouvert en premier, les autres dans l'ordre du serveur. */}
        {me.guilds
          .toSorted((a, b) => Number(b.id === lastGuildId) - Number(a.id === lastGuildId))
          .map((guild) => (
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
