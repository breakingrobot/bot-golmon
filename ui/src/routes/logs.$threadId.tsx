// Log d'un ticket. Adresse conservée telle quelle : les messages Discord y renvoient.

import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { FileText, Paperclip } from 'lucide-react'

import { meQuery, ticketLogQuery } from '@/api/queries'
import type { LogEntry, TicketLog } from '@/api/types'
import { PageContainer } from '@/components/layout'
import { EmptyState, PageSkeleton, QueryError, StatCard } from '@/components/page'
import { RichHtml } from '@/components/rich-html'
import { TicketStatusBadge } from '@/components/ticket-status'
import { Button } from '@/components/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatLongDate, formatNumber, formatTime } from '@/lib/format'
import { compact } from '@/lib/search'
import { cn, initials } from '@/lib/utils'

type Search = { simple?: 1; verbose?: 1 }

const flag = (value: unknown): 1 | undefined => (value === 1 || value === '1' || value === true ? 1 : undefined)

export const Route = createFileRoute('/logs/$threadId')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({ simple: flag(search.simple), verbose: flag(search.verbose) }),
  component: LogPage,
})

const TONE: Record<LogEntry['kind'], { card: string; avatar: string; badge: string }> = {
  user: { card: 'border-l-4 border-l-user', avatar: 'bg-user-soft text-user', badge: 'bg-user-soft text-user' },
  staff: { card: 'border-l-4 border-l-staff', avatar: 'bg-staff-soft text-staff', badge: 'bg-staff-soft text-staff' },
  internal: {
    card: 'border-internal/40 bg-internal-soft/40',
    avatar: 'bg-internal-soft text-internal',
    badge: 'bg-internal-soft text-internal',
  },
  command: { card: 'border-dashed', avatar: 'bg-system-soft text-system', badge: 'bg-system-soft text-system' },
  system: { card: 'bg-surface-sunken', avatar: 'bg-system-soft text-system', badge: 'bg-system-soft text-system' },
  danger: { card: 'border-danger/40', avatar: 'bg-danger-soft text-danger', badge: 'bg-danger-soft text-danger' },
}

function LogPage() {
  const { threadId } = Route.useParams()
  const query = useQuery(ticketLogQuery(threadId))
  return (
    <PageContainer>
      <div className="mx-auto max-w-4xl">
        {query.isPending ? (
          <PageSkeleton stats={3} />
        ) : query.isError ? (
          <QueryError error={query.error} retry={() => void query.refetch()} />
        ) : (
          <Log log={query.data} />
        )}
      </div>
    </PageContainer>
  )
}

function Log({ log }: { log: TicketLog }) {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const me = useQuery(meQuery)
  const view = search.simple ? 'simple' : search.verbose ? 'verbose' : 'all'
  // « Échanges seulement » : messages de l'utilisateur et réponses du staff.
  const entries = search.simple
    ? log.entries.filter((entry) => entry.kind === 'user' || entry.kind === 'staff')
    : log.entries
  const dayKey = (iso: string) => new Date(iso).toDateString()

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
                <LogCard entry={entry} verbose={view === 'verbose'} />
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

function LogCard({ entry, verbose }: { entry: LogEntry; verbose: boolean }) {
  const tone = TONE[entry.kind]
  return (
    <article className={cn('rounded-xl border bg-card px-4 py-3 sm:px-5', tone.card)}>
      <header className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <span
          className={cn(
            'flex size-7 shrink-0 items-center justify-center rounded-full text-xs font-semibold',
            tone.avatar,
          )}
          aria-hidden
        >
          {initials(entry.author).slice(0, 1)}
        </span>
        <span className="font-semibold text-ink">{entry.author}</span>
        {entry.role && <span className="text-sm text-ink-muted">{entry.role}</span>}
        {entry.badge && (
          <span
            className={cn(
              'rounded-md px-1.5 py-0.5 text-[0.6875rem] font-semibold tracking-wide uppercase',
              tone.badge,
            )}
          >
            {entry.badge}
          </span>
        )}
        {entry.number !== null && <span className="font-mono text-xs text-ink-subtle">#{entry.number}</span>}
        <time
          dateTime={entry.at}
          className="ml-auto font-mono text-xs text-ink-subtle"
          title={new Date(entry.at).toLocaleString('fr-FR')}
        >
          {formatTime(entry.at)}
        </time>
      </header>

      {entry.edit ? (
        <div className="mt-3 grid gap-2 sm:grid-cols-2">
          <div className="rounded-lg bg-danger-soft px-3 py-2">
            <p className="text-[0.6875rem] font-semibold tracking-wide text-danger uppercase">Avant</p>
            <RichHtml html={entry.edit.beforeHtml} className="mt-1 text-ink" />
          </div>
          <div className="rounded-lg bg-staff-soft px-3 py-2">
            <p className="text-[0.6875rem] font-semibold tracking-wide text-staff uppercase">Après</p>
            <RichHtml html={entry.edit.afterHtml} className="mt-1 text-ink" />
          </div>
        </div>
      ) : (
        entry.bodyHtml && (
          <RichHtml
            html={entry.bodyHtml}
            className={cn('mt-2', entry.monospace ? 'font-mono text-sm text-ink-muted' : 'text-ink')}
          />
        )
      )}

      {entry.images.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {entry.images.map((image) => (
            <a
              key={image}
              href={image}
              target="_blank"
              rel="noopener noreferrer"
              className="block overflow-hidden rounded-lg border"
            >
              <img
                src={image}
                alt="Pièce jointe"
                loading="lazy"
                className="block max-h-64 min-h-16 max-w-full min-w-24 bg-muted object-contain text-xs text-ink-subtle"
              />
            </a>
          ))}
        </div>
      )}

      {entry.files.length > 0 && (
        <ul className="mt-3 flex flex-wrap gap-2">
          {entry.files.map((file) => (
            <li key={file.url}>
              <a
                href={file.url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex max-w-full items-center gap-2 rounded-lg border bg-muted px-3 py-1.5 text-sm text-ink-muted hover:border-line-strong hover:text-ink"
              >
                <Paperclip className="size-4" aria-hidden />
                <span className="truncate">{file.name}</span>
              </a>
            </li>
          ))}
        </ul>
      )}

      {entry.embeds && (
        <p className="mt-3 rounded-lg border-l-4 border-line-strong bg-muted px-3 py-2 text-sm text-ink-muted">
          {entry.embeds}
        </p>
      )}

      {verbose && entry.technical && (
        <p className="mt-2 font-mono text-xs break-all text-ink-subtle">{entry.technical}</p>
      )}
    </article>
  )
}
