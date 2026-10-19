import { useQuery } from '@tanstack/react-query'
import { getRouteApi, Link, useNavigate } from '@tanstack/react-router'
import { BarChart3, Inbox, Search } from 'lucide-react'
import { useState } from 'react'

import { ticketsQuery } from '@/api/queries'
import { Button } from '@/atoms/Button'
import { Card } from '@/atoms/Card'
import { Input } from '@/atoms/Input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatDateTime, formatNumber, plural } from '@/lib/format'
import { ALL, compact, fromSelect, searchOneOf } from '@/lib/search'
import { cn } from '@/lib/utils'
import { EmptyState } from '@/molecules/EmptyState'
import { Field } from '@/molecules/Field'
import { PageHeader } from '@/molecules/PageHeader'
import { Pagination } from '@/molecules/Pagination'
import { QueryError } from '@/molecules/QueryError'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { TicketStatusBadge } from '@/molecules/TicketStatusBadge'
import { STATUSES } from '@/pages/tickets/tickets-search'

const route = getRouteApi('/tickets/')

export const TicketsView = () => {
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/tickets/' })
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
          const selected = fromSelect(status)
          void navigate({ search: compact({ q: text.trim() || undefined, status: searchOneOf(selected, STATUSES) }) })
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
