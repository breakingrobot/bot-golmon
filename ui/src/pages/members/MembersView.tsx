import { useQuery } from '@tanstack/react-query'
import { getRouteApi, Link, useNavigate } from '@tanstack/react-router'
import { ChevronRight, Search, Users } from 'lucide-react'
import { useEffect, useState } from 'react'

import { memberSearchQuery } from '@/api/queries'
import { Card } from '@/atoms/Card'
import { Input } from '@/atoms/Input'
import { useDebounced } from '@/hooks/use-debounced'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import { cn } from '@/lib/utils'
import { EmptyState } from '@/molecules/EmptyState'
import { MemberAvatar } from '@/molecules/MemberAvatar'
import { PageHeader } from '@/molecules/PageHeader'
import { QueryError } from '@/molecules/QueryError'
import { TableSkeleton } from '@/molecules/TableSkeleton'

const route = getRouteApi('/serveurs/$guildId/membres/')

export const MembersView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/membres/' })
  const [text, setText] = useState(search.q ?? '')
  const q = useDebounced(text.trim(), 250)
  const query = useQuery(memberSearchQuery(guild.id, q))

  useEffect(() => {
    if ((search.q ?? '') !== q) void navigate({ search: compact({ q: q || undefined }), replace: true })
  }, [q, search.q, navigate])

  return (
    <>
      <PageHeader
        title="Membres"
        description={`${guild.name} · présences, cotisations, coffres et recrutement de chacun`}
      />
      <div className="relative max-w-md">
        <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-ink-subtle" aria-hidden />
        <Input
          type="search"
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder="Rechercher un membre…"
          aria-label="Rechercher un membre"
          className="pl-9"
          autoFocus
        />
      </div>
      <p className="mt-2 text-xs text-ink-subtle">25 résultats au plus, parmi les membres connus du bot.</p>
      <div className="mt-4">
        {query.isPending ? (
          <TableSkeleton />
        ) : query.isError ? (
          <QueryError error={query.error} retry={() => void query.refetch()} />
        ) : query.data.length === 0 ? (
          <EmptyState title="Aucun membre trouvé" icon={<Users />} />
        ) : (
          <Card className={cn('gap-0 py-0', query.isPlaceholderData && 'opacity-60')}>
            <ul className="grid divide-y sm:grid-cols-2 sm:divide-y-0">
              {query.data.map((member) => (
                <li key={member.id} className="sm:border-b">
                  <Link
                    to="/serveurs/$guildId/membres/$userId"
                    params={{ guildId: guild.id, userId: member.id }}
                    className="flex items-center gap-3 px-5 py-3 hover:bg-muted/60"
                  >
                    <MemberAvatar member={member} className="size-9" />
                    <span className="min-w-0 flex-1 truncate font-medium text-ink">{member.name}</span>
                    <ChevronRight className="size-4 text-ink-subtle" aria-hidden />
                  </Link>
                </li>
              ))}
            </ul>
          </Card>
        )}
      </div>
    </>
  )
}
