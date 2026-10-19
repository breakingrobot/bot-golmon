// Espace d'un serveur : barre latérale (tiroir sur mobile), sélecteur de serveur, pages autorisées.

import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Outlet, useNavigate } from '@tanstack/react-router'
import { Menu } from 'lucide-react'
import { useMemo, useState } from 'react'

import { meQuery } from '@/api/queries'
import type { GuildAccess, Me } from '@/api/types'
import { GuildPageLink } from '@/components/guild-page-link'
import { PageContainer } from '@/components/layout'
import { LoginLink } from '@/components/login-link'
import { EmptyState, Forbidden, PageSkeleton } from '@/components/page'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet'
import { findGuild, visiblePages } from '@/lib/guild'
import { GuildContext } from '@/lib/guild-context'
import { initials } from '@/lib/utils'

export const Route = createFileRoute('/serveurs/$guildId')({ component: GuildLayout })

function GuildLayout() {
  const { guildId } = Route.useParams()
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

function GuildShell({ me, guild, userId }: { me: Me; guild: GuildAccess; userId: string }) {
  const [open, setOpen] = useState(false)
  const context = useMemo(() => ({ me, guild, userId }), [me, guild, userId])
  return (
    <GuildContext value={context}>
      <div className="mx-auto flex w-full max-w-7xl flex-1">
        <aside className="hidden w-60 shrink-0 border-r lg:block">
          <div className="sticky top-14 max-h-[calc(100dvh-3.5rem)] overflow-y-auto px-3 py-5">
            <GuildNav me={me} guild={guild} userId={userId} />
          </div>
        </aside>
        <div className="min-w-0 flex-1">
          <div className="sticky top-14 z-30 flex items-center gap-2 border-b bg-canvas/95 px-4 py-2 backdrop-blur lg:hidden">
            <Sheet open={open} onOpenChange={setOpen}>
              <SheetTrigger asChild>
                <Button variant="outline" size="sm" aria-label="Ouvrir le menu du serveur">
                  <Menu aria-hidden /> Menu
                </Button>
              </SheetTrigger>
              <SheetContent side="left" className="w-72 overflow-y-auto">
                <SheetHeader>
                  <SheetTitle>{guild.name}</SheetTitle>
                  <SheetDescription className="sr-only">Pages du serveur</SheetDescription>
                </SheetHeader>
                <div className="px-3 pb-6">
                  <GuildNav me={me} guild={guild} userId={userId} onNavigate={() => setOpen(false)} />
                </div>
              </SheetContent>
            </Sheet>
            <span className="truncate text-sm font-medium text-ink">{guild.name}</span>
          </div>
          <main id="contenu" className="px-4 py-6 sm:px-6 lg:px-8">
            <Outlet />
          </main>
        </div>
      </div>
    </GuildContext>
  )
}

function GuildSwitcher({ me, guild }: { me: Me; guild: GuildAccess }) {
  const navigate = useNavigate()
  if (me.guilds.length < 2) {
    return (
      <div className="flex items-center gap-2.5 px-2">
        <GuildBadge guild={guild} />
        <span className="truncate text-sm font-semibold text-ink">{guild.name}</span>
      </div>
    )
  }
  return (
    <Select
      value={guild.id}
      onValueChange={(guildId) => {
        const target = findGuild(me, guildId)
        const first = target ? visiblePages(target)[0] : undefined
        if (first?.to) void navigate({ to: first.to, params: { guildId } })
        else void navigate({ to: '/serveurs/$guildId/membres/$userId', params: { guildId, userId: me.user?.id ?? '' } })
      }}
    >
      <SelectTrigger aria-label="Changer de serveur" className="h-auto py-2">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {me.guilds.map((item) => (
          <SelectItem key={item.id} value={item.id}>
            <GuildBadge guild={item} />
            <span className="truncate font-medium">{item.name}</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

function GuildBadge({ guild }: { guild: GuildAccess }) {
  return guild.iconUrl ? (
    <img src={guild.iconUrl} alt="" className="size-6 rounded-md" />
  ) : (
    <span
      className="flex size-6 shrink-0 items-center justify-center rounded-md bg-brand-soft text-[0.625rem] font-semibold text-brand"
      aria-hidden
    >
      {initials(guild.name)}
    </span>
  )
}

function GuildNav({
  me,
  guild,
  userId,
  onNavigate,
}: {
  me: Me
  guild: GuildAccess
  userId: string
  onNavigate?: () => void
}) {
  const pages = visiblePages(guild)
  const sections = [
    { key: 'serveur', label: 'Serveur', pages: pages.filter((page) => page.section === 'serveur') },
    { key: 'reglages', label: 'Réglages', pages: pages.filter((page) => page.section === 'reglages') },
  ].filter((section) => section.pages.length > 0)
  return (
    <nav aria-label="Pages du serveur" className="space-y-5">
      <GuildSwitcher me={me} guild={guild} />
      {sections.map((section) => (
        <div key={section.key}>
          <p className="mb-1 px-2 text-xs font-medium tracking-wide text-ink-subtle uppercase">{section.label}</p>
          <ul className="space-y-0.5">
            {section.pages.map((page) => (
              <li key={page.key}>
                <GuildPageLink
                  page={page}
                  guildId={guild.id}
                  userId={userId}
                  {...(onNavigate ? { onNavigate } : {})}
                  className="flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm font-medium text-ink-muted transition-colors hover:bg-muted hover:text-ink"
                  activeClassName="bg-brand-soft text-brand hover:bg-brand-soft hover:text-brand"
                >
                  <page.icon className="size-4" aria-hidden />
                  {page.label}
                </GuildPageLink>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </nav>
  )
}
