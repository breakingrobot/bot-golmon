import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { BarChart3, Inbox, Search } from 'lucide-react'
import { useState } from 'react'

import { meQuery, ticketsQuery } from '@/api/queries'
import { Field } from '@/components/form'
import { PageContainer } from '@/components/layout'
import { EmptyState, Forbidden, PageHeader, Pagination, QueryError, TableSkeleton } from '@/components/page'
import { TicketStatusBadge } from '@/components/ticket-status'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { formatDateTime, formatNumber, plural } from '@/lib/format'
import { ALL, compact, fromSelect, searchOneOf, searchPage, searchString } from '@/lib/search'
import { cn } from '@/lib/utils'

const STATUSES = ['open', 'closed', 'suspended'] as const
type Search = { q?: string; status?: (typeof STATUSES)[number]; page?: number }

export const Route = createFileRoute('/tickets/')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({
      q: searchString(search.q),
      status: searchOneOf(search.status, STATUSES),
      page: searchPage(search.page),
    }),
  component: TicketsPage,
})

function TicketsPage() {
  const me = useQuery(meQuery)
  return (
    <PageContainer>
      {me.data && !me.data.tickets ? <Forbidden>Réservé au staff des tickets.</Forbidden> : <Tickets />}
    </PageContainer>
  )
}

function Tickets() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const [text, setText] = useState(search.q ?? '')
  const [status, setStatus] = useState<string>(search.status ?? ALL)
  const query = useQuery(ticketsQuery(compact({ q: search.q, status: search.status }), search.page ?? 1))

  return (
    <>
      <PageHeader
        title="Tickets"
        description={query.data ? plural(query.data.total, 'ticket') : undefined}
        actions={
          <Button asChild variant="outline" size="sm">
            <Link to="/tickets/staff">
              <BarChart3 aria-hidden /> Statistiques du staff
            </Link>
          </Button>
        }
      />
      <form
        className="grid gap-3 rounded-xl border bg-card p-4 shadow-xs sm:grid-cols-[1fr_12rem_auto] sm:items-end"
        onSubmit={(event) => {
          event.preventDefault()
          void navigate({
            search: compact({ q: text.trim() || undefined, status: searchOneOf(fromSelect(status), STATUSES) }),
          })
        }}
      >
        <Field label="Rechercher">
          {(props) => (
            <div className="relative">
              <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-ink-subtle" aria-hidden />
              <Input
                {...props}
                type="search"
                value={text}
                onChange={(event) => setText(event.target.value)}
                placeholder="Pseudo, identifiant, n° de ticket ou texte d'un message"
                className="pl-9"
              />
            </div>
          )}
        </Field>
        <Field label="Statut">
          {(props) => (
            <Select value={status} onValueChange={setStatus}>
              <SelectTrigger {...props}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>Tous</SelectItem>
                <SelectItem value="open">Ouverts</SelectItem>
                <SelectItem value="suspended">Suspendus</SelectItem>
                <SelectItem value="closed">Fermés</SelectItem>
              </SelectContent>
            </Select>
          )}
        </Field>
        <Button type="submit">Rechercher</Button>
      </form>

      <div className="mt-6">
        {query.isPending ? (
          <TableSkeleton rows={10} />
        ) : query.isError ? (
          <QueryError error={query.error} retry={() => void query.refetch()} />
        ) : query.data.items.length === 0 ? (
          <EmptyState title="Aucun ticket trouvé" icon={<Inbox />} />
        ) : (
          <>
            <Card className={cn('py-0', query.isPlaceholderData && 'opacity-60')}>
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead className="w-16">N°</TableHead>
                    <TableHead>Utilisateur</TableHead>
                    <TableHead>Pris en charge</TableHead>
                    <TableHead className="text-right">Messages</TableHead>
                    <TableHead>Ouvert le</TableHead>
                    <TableHead>Statut</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {query.data.items.map((ticket) => (
                    <TableRow key={ticket.id} className="relative">
                      <TableCell className="font-mono text-ink-subtle">
                        <Link
                          to="/logs/$threadId"
                          params={{ threadId: ticket.id }}
                          className="after:absolute after:inset-0 focus-visible:outline-none focus-visible:after:ring-[3px] focus-visible:after:ring-ring/50"
                          aria-label={`Ticket n° ${ticket.number} de ${ticket.user.name}`}
                        >
                          #{ticket.number}
                        </Link>
                      </TableCell>
                      <TableCell>
                        <span className="font-medium text-ink">{ticket.user.name}</span>
                        <span className="ml-2 hidden font-mono text-xs text-ink-subtle md:inline">
                          {ticket.user.id}
                        </span>
                      </TableCell>
                      <TableCell className={ticket.claimedBy ? 'text-staff' : 'text-ink-subtle'}>
                        {ticket.claimedBy ?? '—'}
                      </TableCell>
                      <TableCell className="text-right text-ink-muted tabular-nums">
                        {formatNumber(ticket.exchanges)}
                      </TableCell>
                      <TableCell className="text-ink-muted tabular-nums">{formatDateTime(ticket.openedAt)}</TableCell>
                      <TableCell>
                        <TicketStatusBadge status={ticket.status} />
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </Card>
            <Pagination
              page={query.data.page}
              pageSize={query.data.pageSize}
              total={query.data.total}
              onPage={(page) =>
                void navigate({ search: (prev) => compact({ ...prev, page: page > 1 ? page : undefined }) })
              }
            />
          </>
        )}
      </div>
    </>
  )
}
