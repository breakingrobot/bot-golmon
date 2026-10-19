import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Coins, X } from 'lucide-react'

import { csvUrl } from '@/api/client'
import { cotisationsQuery, memberProfileQuery, type CotisationFilters } from '@/api/queries'
import type { CotisationsResponse, MemberRef } from '@/api/types'
import { WeeklyCotisationsChart } from '@/components/charts/charts'
import { AddCotisationDialog } from '@/components/cotisation-dialog'
import { EntriesTable } from '@/components/cotisation-entries'
import { CsvButton } from '@/components/form'
import { MemberName } from '@/components/member'
import { MemberPicker } from '@/components/member-picker'
import { COUNTS_LABEL, ObjectiveBar, ObjectiveStatus } from '@/components/objective'
import {
  EmptyState,
  Forbidden,
  PageHeader,
  PageSkeleton,
  Pagination,
  QueryError,
  Section,
  ShareBar,
  StatCard,
  StatGrid,
} from '@/components/page'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatMoney, formatNumber, formatPercent } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { compact, searchOneOf, searchPage, searchString } from '@/lib/search'
import { cn } from '@/lib/utils'

const PERIODS = [
  { value: '7', label: '7 jours' },
  { value: '30', label: '30 jours' },
  { value: '90', label: '90 jours' },
  { value: '365', label: '1 an' },
  { value: 'tout', label: 'Tout' },
] as const

type Period = (typeof PERIODS)[number]['value']
type Search = { jours?: Period; membre?: string; page?: number }

export const Route = createFileRoute('/serveurs/$guildId/cotisations')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({
      jours: searchOneOf(
        search.jours,
        PERIODS.map((period) => period.value),
      ),
      membre: searchString(search.membre),
      page: searchPage(search.page),
    }),
  component: CotisationsPage,
})

function CotisationsPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'cotisation.stats')) return <Forbidden />
  return <Cotisations />
}

function Cotisations() {
  const { guild } = useGuild()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const jours = search.jours ?? '30'
  const filters: CotisationFilters = { jours, ...(search.membre ? { membre: search.membre } : {}) }
  const query = useQuery(cotisationsQuery(guild.id, filters, search.page ?? 1))
  const canAdd = hasPerm(guild, 'cotisation.ajouter')
  const canManage = hasPerm(guild, 'cotisation.gerer')

  return (
    <>
      <PageHeader
        title="Cotisations"
        description={guild.name}
        actions={
          <>
            <CsvButton href={csvUrl(`/guilds/${guild.id}/cotisations/export.csv`, filters)} />
            {canAdd && <AddCotisationDialog guildId={guild.id} canManage={canManage} />}
          </>
        }
      />

      <div className="flex flex-wrap items-center gap-3">
        <ToggleGroup
          type="single"
          value={jours}
          onValueChange={(value) => {
            if (value)
              void navigate({ search: (prev) => compact({ ...prev, jours: value as Period, page: undefined }) })
          }}
          aria-label="Période"
        >
          {PERIODS.map((period) => (
            <ToggleGroupItem key={period.value} value={period.value}>
              {period.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <MemberFilter
          guildId={guild.id}
          memberId={search.membre}
          known={query.data?.members.map((row) => row.member) ?? []}
          onChange={(membre) => void navigate({ search: (prev) => compact({ ...prev, membre, page: undefined }) })}
        />
      </div>

      {query.isPending ? (
        <div className="mt-6">
          <PageSkeleton />
        </div>
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <div className={cn('transition-opacity', query.isPlaceholderData && 'opacity-60')}>
          <CotisationsContent
            data={query.data}
            filteredMember={Boolean(search.membre)}
            onMember={(membre) => void navigate({ search: (prev) => compact({ ...prev, membre, page: undefined }) })}
            onPage={(page) =>
              void navigate({ search: (prev) => compact({ ...prev, page: page > 1 ? page : undefined }) })
            }
          />
        </div>
      )}
    </>
  )
}

function MemberFilter({
  guildId,
  memberId,
  known,
  onChange,
}: {
  guildId: string
  memberId: string | undefined
  known: MemberRef[]
  onChange: (memberId: string | undefined) => void
}) {
  // Nom du membre filtré : pris dans le classement, sinon dans sa fiche.
  const fromList = known.find((item) => item.id === memberId)
  const profile = useQuery({ ...memberProfileQuery(guildId, memberId ?? ''), enabled: Boolean(memberId) && !fromList })
  const current = fromList ?? profile.data?.member ?? null
  return (
    <div className="flex min-w-56 items-center gap-1">
      <div className="min-w-0 flex-1">
        <MemberPicker
          guildId={guildId}
          value={current}
          onChange={(member) => onChange(member?.id)}
          placeholder="Tous les membres"
        />
      </div>
      {memberId && (
        <Button variant="ghost" size="icon-sm" onClick={() => onChange(undefined)} aria-label="Voir tous les membres">
          <X aria-hidden />
        </Button>
      )}
    </div>
  )
}

function CotisationsContent({
  data,
  filteredMember,
  onMember,
  onPage,
}: {
  data: CotisationsResponse
  filteredMember: boolean
  onMember: (memberId: string) => void
  onPage: (page: number) => void
}) {
  const { guild } = useGuild()
  const total = data.totals.propre + data.totals.sale
  return (
    <>
      <StatGrid className="mt-6 lg:grid-cols-5">
        <StatCard label="Total" value={formatMoney(total)} tone="staff" />
        <StatCard label="Argent propre" value={formatMoney(data.totals.propre)} />
        <StatCard label="Argent sale" value={formatMoney(data.totals.sale)} tone="internal" />
        <StatCard label="Cotisations" value={formatNumber(data.totals.count)} />
        <StatCard label="Membres" value={formatNumber(data.members.length)} hint="ayant cotisé" />
      </StatGrid>

      <Section title="Par semaine" description="Argent propre et sale, lundi → dimanche (heure de Paris).">
        <Card className="pb-3">
          <CardContent>
            <WeeklyCotisationsChart weekly={data.weekly} />
          </CardContent>
        </Card>
      </Section>

      {data.objective && (
        <Section
          title="Objectif de la semaine"
          description={`${formatMoney(data.objective.amount)} par membre (${COUNTS_LABEL[data.objective.counts]})${data.objective.reminder ? ', rappel le dimanche' : ''}. Du plus loin au plus proche de l'objectif.`}
        >
          {data.objectiveProgress.length === 0 ? (
            <EmptyState title="Aucun membre suivi" />
          ) : (
            <Card className="gap-0 py-0">
              <ul className="grid divide-y sm:grid-cols-2 sm:divide-y-0 lg:grid-cols-3">
                {data.objectiveProgress.map((progress) => (
                  <li key={progress.member.id} className="grid gap-2 px-5 py-3 sm:border-b">
                    <div className="flex items-center justify-between gap-2">
                      <MemberName member={progress.member} guildId={guild.id} />
                      <ObjectiveStatus progress={progress} />
                    </div>
                    <ObjectiveBar progress={progress} />
                  </li>
                ))}
              </ul>
            </Card>
          )}
        </Section>
      )}

      {!filteredMember && (
        <Section title="Classement" description="Cliquez sur un membre pour ne voir que ses cotisations.">
          {data.members.length === 0 ? (
            <EmptyState title="Aucune cotisation sur la période" icon={<Coins />} />
          ) : (
            <Card className="py-0">
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead className="w-10">#</TableHead>
                    <TableHead>Membre</TableHead>
                    <TableHead>Part</TableHead>
                    <TableHead className="text-right">Propre</TableHead>
                    <TableHead className="text-right">Sale</TableHead>
                    <TableHead className="text-right">Total</TableHead>
                    <TableHead className="text-right">Cotis.</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.members.map((row, index) => {
                    const sum = row.propre + row.sale
                    const share = total > 0 ? Math.round((sum / total) * 100) : 0
                    return (
                      <TableRow key={row.member.id}>
                        <TableCell className="text-ink-subtle tabular-nums">{index + 1}</TableCell>
                        <TableCell>
                          <button
                            type="button"
                            className="rounded-sm text-left hover:text-brand hover:underline focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
                            onClick={() => onMember(row.member.id)}
                            aria-label={`Voir les cotisations de ${row.member.name}`}
                          >
                            <MemberName member={row.member} linked={false} className="hover:text-brand" />
                          </button>
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-2">
                            <ShareBar value={share} label={`Part de ${row.member.name}`} className="w-20" />
                            <span className="text-ink-muted tabular-nums">{formatPercent(share)}</span>
                          </div>
                        </TableCell>
                        <TableCell className="text-right tabular-nums">{formatMoney(row.propre)}</TableCell>
                        <TableCell className="text-right text-internal tabular-nums">{formatMoney(row.sale)}</TableCell>
                        <TableCell className="text-right font-semibold text-staff tabular-nums">
                          {formatMoney(sum)}
                        </TableCell>
                        <TableCell className="text-right text-ink-muted tabular-nums">
                          {formatNumber(row.count)}
                        </TableCell>
                      </TableRow>
                    )
                  })}
                </TableBody>
              </Table>
            </Card>
          )}
        </Section>
      )}

      <Section
        title="Détail"
        description={`${formatNumber(data.entries.total)} cotisation(s), des plus récentes aux plus anciennes.`}
      >
        {data.entries.items.length === 0 ? (
          <EmptyState title="Aucune cotisation" icon={<Coins />} />
        ) : (
          <Card className="py-0">
            <EntriesTable entries={data.entries.items} />
          </Card>
        )}
        <Pagination
          page={data.entries.page}
          pageSize={data.entries.pageSize}
          total={data.entries.total}
          onPage={onPage}
          newerLabel="Plus récentes"
          olderLabel="Plus anciennes"
        />
      </Section>
    </>
  )
}
