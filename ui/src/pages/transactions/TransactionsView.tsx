import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { Receipt } from 'lucide-react'

import { csvUrl } from '@/api/client'
import { transactionsQuery } from '@/api/queries'
import { Card, CardContent } from '@/atoms/Card'
import { formatMoney, formatNumber, formatSignedMoney } from '@/lib/format'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import { cn } from '@/lib/utils'
import { CsvButton } from '@/molecules/CsvButton'
import { EmptyState } from '@/molecules/EmptyState'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { Pagination } from '@/molecules/Pagination'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { StatCard } from '@/molecules/StatCard'
import { StatGrid } from '@/molecules/StatGrid'
import { MonthlyTransactionsChart } from '@/organisms/MonthlyTransactionsChart'
import { TransactionFiltersForm } from '@/pages/transactions/TransactionFiltersForm'
import { TransactionTable } from '@/pages/transactions/TransactionTable'

const route = getRouteApi('/serveurs/$guildId/transactions')

export const TransactionsView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/transactions' })
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
