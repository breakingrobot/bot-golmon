import { useQuery } from '@tanstack/react-query'
import { getRouteApi, Link, useNavigate } from '@tanstack/react-router'
import { FileText } from 'lucide-react'

import { meQuery } from '@/api/queries'
import type { TicketLog } from '@/api/types'
import { Button } from '@/atoms/Button'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { formatLongDate, formatNumber, formatTime } from '@/lib/format'
import { compact } from '@/lib/search'
import { EmptyState } from '@/molecules/EmptyState'
import { StatCard } from '@/molecules/StatCard'
import { TicketStatusBadge } from '@/molecules/TicketStatusBadge'
import { LogEntryCard } from '@/organisms/LogEntryCard'

const route = getRouteApi('/logs/$threadId')

export type LogViewProps = { log: TicketLog }

/** Jour local d'un horodatage, pour les séparateurs de jours. */
const dayKey = (iso: string) => new Date(iso).toDateString()

export const LogView = ({ log }: LogViewProps) => {
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/logs/$threadId' })
  const me = useQuery(meQuery)
  const view = search.simple ? 'simple' : search.verbose ? 'verbose' : 'all'
  // « Échanges seulement » : messages de l'utilisateur et réponses du staff.
  const entries = search.simple
    ? log.entries.filter((entry) => entry.kind === 'user' || entry.kind === 'staff')
    : log.entries

  return (
    <>
      {me.data?.tickets && (
        <Button asChild variant="link" className="mb-2 h-auto px-0">
          <Link to="/tickets">← Tickets</Link>
        </Button>
      )}
      <header className="mb-8">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <h1 className="text-2xl font-semibold tracking-tight text-ink sm:text-3xl">Ticket #{log.number}</h1>
          <TicketStatusBadge status={log.status} />
        </div>
        <p className="mt-2 text-ink-muted">
          Avec <span className="font-medium text-ink">{log.user.name}</span>{' '}
          <span className="font-mono text-sm text-ink-subtle">({log.user.id})</span> · ouvert le{' '}
          <time dateTime={log.openedAt}>
            {formatLongDate(log.openedAt)} à {formatTime(log.openedAt)}
          </time>
        </p>
        <dl className="mt-6 grid grid-cols-3 gap-3">
          <StatCard label="Utilisateur" value={formatNumber(log.counts.fromUser)} tone="user" />
          <StatCard label="Staff" value={formatNumber(log.counts.toUser)} tone="staff" />
          <StatCard label="Notes internes" value={formatNumber(log.counts.internal)} tone="internal" />
        </dl>
        <div className="mt-6 flex flex-wrap items-center gap-2">
          <ToggleGroup
            type="single"
            value={view}
            aria-label="Affichage du log"
            onValueChange={(value) => {
              if (!value) return
              void navigate({
                search: compact({
                  simple: value === 'simple' ? (1 as const) : undefined,
                  verbose: value === 'verbose' ? (1 as const) : undefined,
                }),
                replace: true,
              })
            }}
          >
            <ToggleGroupItem value="all">Tout</ToggleGroupItem>
            <ToggleGroupItem value="simple">Échanges seulement</ToggleGroupItem>
            <ToggleGroupItem value="verbose">Détails techniques</ToggleGroupItem>
          </ToggleGroup>
          <Button asChild variant="ghost" size="sm" className="ml-auto">
            {/* Texte brut servi par le bot (hors SPA) : navigation complète. */}
            <a href={log.rawUrl}>
              <FileText aria-hidden /> Texte brut
            </a>
          </Button>
        </div>
      </header>

      {entries.length === 0 ? (
        <EmptyState title="Aucun message à afficher." />
      ) : (
        <ol className="space-y-3">
          {entries.map((entry, index) => {
            const previous = entries[index - 1]
            const newDay = !previous || dayKey(previous.at) !== dayKey(entry.at)
            return (
              <li key={entry.id}>
                {newDay && (
                  <div className="flex items-center gap-3 pt-4 pb-3" aria-hidden>
                    <span className="h-px flex-1 bg-line" />
                    <span className="text-xs font-medium tracking-wide text-ink-subtle uppercase">
                      {formatLongDate(entry.at)}
                    </span>
                    <span className="h-px flex-1 bg-line" />
                  </div>
                )}
                <LogEntryCard entry={entry} verbose={view === 'verbose'} />
              </li>
            )
          })}
        </ol>
      )}

      <footer className="mt-12 border-t pt-6 text-sm text-ink-subtle">
        Heures affichées dans votre fuseau horaire.
      </footer>
    </>
  )
}
