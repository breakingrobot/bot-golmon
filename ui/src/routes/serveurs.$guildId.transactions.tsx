import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ChevronRight, Filter, Receipt, X } from 'lucide-react'
import { Fragment, useState } from 'react'

import { csvUrl } from '@/api/client'
import { transactionsQuery, type TransactionFilters } from '@/api/queries'
import type { Transaction, TransactionStatus } from '@/api/types'
import { MonthlyTransactionsChart } from '@/components/charts/charts'
import { CsvButton, Field, FilterBar } from '@/components/form'
import { MemberName } from '@/components/member'
import {
  EmptyState,
  Forbidden,
  PageHeader,
  PageSkeleton,
  Pagination,
  QueryError,
  Section,
  StatCard,
  StatGrid,
} from '@/components/page'
import { Badge, type BadgeVariant } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { formatDateTime, formatMoney, formatNumber, formatSignedMoney } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect, searchDay, searchOneOf, searchPage, searchString } from '@/lib/search'
import { cn } from '@/lib/utils'

type Search = TransactionFilters & { page?: number }

export const Route = createFileRoute('/serveurs/$guildId/transactions')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({
      groupe: searchString(search.groupe),
      statut: searchString(search.statut),
      type: searchOneOf(search.type, ['ACHAT', 'VENTE'] as const),
      du: searchDay(search.du),
      au: searchDay(search.au),
      page: searchPage(search.page),
    }),
  component: TransactionsPage,
})

function TransactionsPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'transaction.historique')) return <Forbidden />
  return <Transactions />
}

const STATUS_VARIANT: Record<TransactionStatus, BadgeVariant> = {
  PREPARATION: 'neutral',
  EN_ATTENTE_RDV: 'warning',
  AVANCE_ATTENTE_PAIEMENT: 'warning',
  LIVRE_PAYE: 'success',
  ANNULEE: 'danger',
  CLOTUREE: 'neutral',
}

function Transactions() {
  const { guild } = useGuild()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const { page = 1, ...filters } = search
  const query = useQuery(transactionsQuery(guild.id, filters, page))
  const filtered = Object.keys(filters).length > 0

  return (
    <>
      <PageHeader
        title="Transactions"
        description={`${guild.name} · historique des achats et ventes`}
        actions={<CsvButton href={csvUrl(`/guilds/${guild.id}/transactions/export.csv`, filters)} />}
      />
      <TransactionFiltersForm
        key={JSON.stringify(filters)}
        initial={filters}
        statuses={query.data?.statuses ?? []}
        onApply={(next) => void navigate({ search: compact(next) })}
        onReset={filtered ? () => void navigate({ search: {} }) : undefined}
      />

      {query.isPending ? (
        <div className="mt-6">
          <PageSkeleton stats={5} />
        </div>
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <div className={cn('transition-opacity', query.isPlaceholderData && 'opacity-60')}>
          <StatGrid className="mt-6 lg:grid-cols-5">
            <StatCard label="Transactions" value={formatNumber(query.data.totals.count)} hint="annulées comprises" />
            <StatCard label="Achats" value={formatMoney(query.data.totals.achats)} />
            <StatCard label="Ventes" value={formatMoney(query.data.totals.ventes)} />
            <StatCard
              label="Solde"
              value={formatSignedMoney(query.data.totals.solde)}
              tone={query.data.totals.solde >= 0 ? 'staff' : 'danger'}
              hint="ventes − achats"
            />
            <StatCard label="En cours" value={formatNumber(query.data.totals.inProgress)} tone="internal" />
          </StatGrid>

          {query.data.months.length > 0 && (
            <Section title="Par mois" description="12 derniers mois de la sélection, hors transactions annulées.">
              <Card className="pb-3">
                <CardContent>
                  <MonthlyTransactionsChart months={query.data.months} />
                </CardContent>
              </Card>
            </Section>
          )}

          <Section
            title="Détail"
            description={`${formatNumber(query.data.list.total)} transaction(s), des plus récentes aux plus anciennes. Cliquez sur une ligne pour voir ses objets.`}
          >
            {query.data.list.items.length === 0 ? (
              <EmptyState title="Aucune transaction" icon={<Receipt />}>
                {filtered ? 'Aucune transaction ne correspond aux filtres.' : 'Aucune transaction enregistrée.'}
              </EmptyState>
            ) : (
              <Card className="py-0">
                <TransactionTable items={query.data.list.items} guildId={guild.id} />
              </Card>
            )}
            <Pagination
              page={query.data.list.page}
              pageSize={query.data.list.pageSize}
              total={query.data.list.total}
              onPage={(next) =>
                void navigate({ search: (prev) => compact({ ...prev, page: next > 1 ? next : undefined }) })
              }
              newerLabel="Plus récentes"
              olderLabel="Plus anciennes"
            />
          </Section>
        </div>
      )}
    </>
  )
}

function TransactionFiltersForm({
  initial,
  statuses,
  onApply,
  onReset,
}: {
  initial: TransactionFilters
  statuses: { value: TransactionStatus; label: string }[]
  onApply: (filters: TransactionFilters) => void
  onReset: (() => void) | undefined
}) {
  const [groupe, setGroupe] = useState(initial.groupe ?? '')
  const [statut, setStatut] = useState(initial.statut ?? ALL)
  const [type, setType] = useState(initial.type ?? ALL)
  const [du, setDu] = useState(initial.du ?? '')
  const [au, setAu] = useState(initial.au ?? '')
  return (
    <FilterBar
      onSubmit={() =>
        onApply(
          compact({
            groupe: groupe.trim() || undefined,
            statut: fromSelect(statut),
            type: fromSelect(type),
            du: du || undefined,
            au: au || undefined,
          }),
        )
      }
    >
      <Field label="Groupe">
        {(props) => (
          <Input
            {...props}
            type="search"
            value={groupe}
            onChange={(event) => setGroupe(event.target.value)}
            placeholder="Nom (partiel)"
          />
        )}
      </Field>
      <Field label="Statut">
        {(props) => (
          <Select value={statut} onValueChange={setStatut}>
            <SelectTrigger {...props}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL}>Tous les statuts</SelectItem>
              {statuses.map((status) => (
                <SelectItem key={status.value} value={status.value}>
                  {status.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </Field>
      <Field label="Type">
        {(props) => (
          <Select value={type} onValueChange={setType}>
            <SelectTrigger {...props}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL}>Achats et ventes</SelectItem>
              <SelectItem value="ACHAT">Achats</SelectItem>
              <SelectItem value="VENTE">Ventes</SelectItem>
            </SelectContent>
          </Select>
        )}
      </Field>
      <Field label="Du">
        {(props) => (
          <Input
            {...props}
            type="date"
            value={du}
            max={au || undefined}
            onChange={(event) => setDu(event.target.value)}
          />
        )}
      </Field>
      <Field label="Au">
        {(props) => (
          <Input
            {...props}
            type="date"
            value={au}
            min={du || undefined}
            onChange={(event) => setAu(event.target.value)}
          />
        )}
      </Field>
      <div className="flex items-end gap-2">
        <Button type="submit">
          <Filter aria-hidden /> Filtrer
        </Button>
        {onReset && (
          <Button type="button" variant="ghost" onClick={onReset}>
            <X aria-hidden /> Réinitialiser
          </Button>
        )}
      </div>
    </FilterBar>
  )
}

function TransactionTable({ items, guildId }: { items: Transaction[]; guildId: string }) {
  const [open, setOpen] = useState<string | null>(null)
  return (
    <Table>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className="w-8">
            <span className="sr-only">Objets</span>
          </TableHead>
          <TableHead>Date</TableHead>
          <TableHead>Groupe</TableHead>
          <TableHead>Type</TableHead>
          <TableHead>Statut</TableHead>
          <TableHead className="text-right">Prix</TableHead>
          <TableHead>Créée par</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {items.map((item) => {
          const expanded = open === item.id
          return (
            <Fragment key={item.id}>
              <TableRow
                className={cn('cursor-pointer', expanded && 'bg-muted/50')}
                onClick={() => setOpen(expanded ? null : item.id)}
              >
                <TableCell className="pr-0">
                  <button
                    type="button"
                    aria-expanded={expanded}
                    aria-controls={`objets-${item.id}`}
                    aria-label={`${expanded ? 'Masquer' : 'Afficher'} les objets de la transaction avec ${item.groupe}`}
                    className="flex size-6 items-center justify-center rounded-sm text-ink-subtle hover:text-ink focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
                    onClick={(event) => {
                      event.stopPropagation()
                      setOpen(expanded ? null : item.id)
                    }}
                  >
                    <ChevronRight className={cn('size-4 transition-transform', expanded && 'rotate-90')} aria-hidden />
                  </button>
                </TableCell>
                <TableCell className="text-ink-muted tabular-nums">{formatDateTime(item.createdAt)}</TableCell>
                <TableCell className="font-medium">{item.groupe}</TableCell>
                <TableCell>
                  <Badge variant={item.type === 'ACHAT' ? 'brand' : 'success'}>
                    {item.type === 'ACHAT' ? 'Achat' : 'Vente'}
                  </Badge>
                </TableCell>
                <TableCell>
                  <Badge variant={STATUS_VARIANT[item.status]}>{item.statusLabel}</Badge>
                </TableCell>
                <TableCell
                  className={cn(
                    'text-right font-medium tabular-nums',
                    item.status === 'ANNULEE' && 'text-ink-subtle line-through',
                  )}
                >
                  {formatMoney(item.prix, true)}
                </TableCell>
                <TableCell>
                  <MemberName member={item.creator} guildId={guildId} linked={false} />
                </TableCell>
              </TableRow>
              {expanded && (
                <TableRow id={`objets-${item.id}`} className="bg-muted/30 hover:bg-muted/30">
                  <TableCell />
                  <TableCell colSpan={6} className="py-3">
                    {item.items.length === 0 ? (
                      <p className="text-sm text-ink-muted">Aucun objet saisi.</p>
                    ) : (
                      <ul className="grid gap-1 text-sm sm:max-w-md">
                        {item.items.map((line, index) => (
                          // oxlint-disable-next-line react/no-array-index-key -- les lignes d'objets n'ont pas d'identifiant
                          <li key={`${line.nom}-${index}`} className="flex justify-between gap-4">
                            <span>
                              <span className="font-medium">{line.nom}</span>{' '}
                              <span className="text-ink-muted">× {formatNumber(line.quantite)}</span>
                            </span>
                            <span className="text-ink-muted tabular-nums">
                              {line.prixUnitaire === null
                                ? 'prix non précisé'
                                : `${formatMoney(line.prixUnitaire, true)} / u`}
                            </span>
                          </li>
                        ))}
                      </ul>
                    )}
                  </TableCell>
                </TableRow>
              )}
            </Fragment>
          )
        })}
      </TableBody>
    </Table>
  )
}
