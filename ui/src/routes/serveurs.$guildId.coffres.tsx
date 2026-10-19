import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { PackageOpen, TriangleAlert, X } from 'lucide-react'

import { csvUrl } from '@/api/client'
import {
  coffreMetaQuery,
  coffreMovementsQuery,
  coffrePlayersQuery,
  coffreSeriesQuery,
  coffreStockQuery,
  type CoffreFilters,
} from '@/api/queries'
import type { CoffreMeta, CoffreStock } from '@/api/types'
import { CoffreSeriesChart } from '@/components/charts/charts'
import {
  LinkCharacterDialog,
  propertyLabel,
  RenamePropertyDialog,
  UnlinkCharacterButton,
} from '@/components/coffre-dialogs'
import { MovementsTable } from '@/components/coffre-movements'
import { CsvButton, Field } from '@/components/form'
import { MemberName } from '@/components/member'
import { EmptyState, Forbidden, PageHeader, Pagination, QueryError, Section, TableSkeleton } from '@/components/page'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/misc'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatNumber } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect, searchOneOf, searchPage, searchString, type SearchPatch } from '@/lib/search'
import { cn } from '@/lib/utils'

const PERIODS = [
  { value: '1', label: '24 h' },
  { value: '7', label: '7 jours' },
  { value: '30', label: '30 jours' },
  { value: '90', label: '90 jours' },
  { value: 'tout', label: 'Tout' },
] as const

type Period = (typeof PERIODS)[number]['value']
type Search = {
  propriete?: string
  objet?: string
  joueur?: string
  type?: 'deposit' | 'withdraw'
  jours?: Period
  page?: number
}

export const Route = createFileRoute('/serveurs/$guildId/coffres')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({
      propriete: searchString(search.propriete),
      objet: searchString(search.objet),
      joueur: searchString(search.joueur),
      type: searchOneOf(search.type, ['deposit', 'withdraw'] as const),
      jours: searchOneOf(
        search.jours,
        PERIODS.map((period) => period.value),
      ),
      page: searchPage(search.page),
    }),
  component: CoffresPage,
})

function CoffresPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'coffre.voir')) return <Forbidden />
  return <Coffres />
}

function Coffres() {
  const { guild } = useGuild()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const { page = 1, ...rest } = search
  const filters: CoffreFilters = { ...rest, jours: rest.jours ?? '30' }
  const meta = useQuery(coffreMetaQuery(guild.id))
  const setFilter = (patch: SearchPatch<Search>) =>
    void navigate({ search: (prev) => compact({ ...prev, ...patch, page: undefined }) })
  const filtered = Boolean(search.propriete || search.objet || search.joueur || search.type)
  const itemLabel = meta.data?.items.find((item) => item.item === search.objet)?.label ?? search.objet

  return (
    <>
      <PageHeader
        title="Coffres"
        description={`${guild.name} · stock, activité par personnage, mouvements`}
        actions={<CsvButton href={csvUrl(`/guilds/${guild.id}/coffres/export.csv`, filters)} />}
      />

      <div className="grid gap-3 rounded-xl border bg-card p-4 shadow-xs sm:grid-cols-2 lg:grid-cols-4">
        <Field label="Propriété">
          {(props) => (
            <Select
              value={search.propriete ?? ALL}
              onValueChange={(value) => setFilter({ propriete: fromSelect(value) })}
            >
              <SelectTrigger {...props}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>Toutes les propriétés</SelectItem>
                {meta.data?.properties.map((property) => (
                  <SelectItem key={property.id} value={property.id}>
                    {propertyLabel(property)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </Field>
        <Field label="Objet">
          {(props) => (
            <Select value={search.objet ?? ALL} onValueChange={(value) => setFilter({ objet: fromSelect(value) })}>
              <SelectTrigger {...props}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>Tous les objets</SelectItem>
                {meta.data?.items.map((item) => (
                  <SelectItem key={item.item} value={item.item}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </Field>
        <Field label="Type">
          {(props) => (
            <Select
              value={search.type ?? ALL}
              onValueChange={(value) =>
                setFilter({ type: value === 'deposit' || value === 'withdraw' ? value : undefined })
              }
            >
              <SelectTrigger {...props}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>Dépôts et retraits</SelectItem>
                <SelectItem value="deposit">Dépôts</SelectItem>
                <SelectItem value="withdraw">Retraits</SelectItem>
              </SelectContent>
            </Select>
          )}
        </Field>
        <PlayerSelect
          guildId={guild.id}
          meta={meta.data}
          value={search.joueur}
          jours={filters.jours}
          onChange={(joueur) => setFilter({ joueur })}
        />
        <div className="flex flex-wrap items-center gap-2 sm:col-span-2 lg:col-span-4">
          <ToggleGroup
            type="single"
            value={filters.jours}
            onValueChange={(value) => value && setFilter({ jours: value as Period })}
            aria-label="Période"
          >
            {PERIODS.map((period) => (
              <ToggleGroupItem key={period.value} value={period.value}>
                {period.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          {filtered && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void navigate({ search: compact({ jours: search.jours }) })}
            >
              <X aria-hidden /> Réinitialiser
            </Button>
          )}
        </div>
      </div>

      {meta.isError && <QueryError error={meta.error} retry={() => void meta.refetch()} />}

      <StockSection guildId={guild.id} propriete={search.propriete} canManage={hasPerm(guild, 'coffre.gerer')} />

      {search.objet && (
        <SeriesSection
          guildId={guild.id}
          objet={search.objet}
          itemLabel={itemLabel ?? search.objet}
          propriete={search.propriete}
          jours={filters.jours}
        />
      )}

      <PlayersSection
        guildId={guild.id}
        filters={filters}
        meta={meta.data}
        canManage={hasPerm(guild, 'coffre.gerer')}
      />

      <MovementsSection
        guildId={guild.id}
        filters={filters}
        page={page}
        onPage={(next) => void navigate({ search: (prev) => compact({ ...prev, page: next > 1 ? next : undefined }) })}
      />
    </>
  )
}

function PlayerSelect({
  guildId,
  meta,
  value,
  jours,
  onChange,
}: {
  guildId: string
  meta: CoffreMeta | undefined
  value: string | undefined
  jours: string
  onChange: (joueur: string | undefined) => void
}) {
  // Personnages vus sur la période, plus les personnages reliés à un membre.
  const players = useQuery(coffrePlayersQuery(guildId, { jours }))
  const options = new Map<string, string>()
  for (const link of meta?.links ?? []) options.set(link.gameId, `${link.name} (${link.member.name})`)
  for (const player of players.data ?? []) {
    if (player.gameId && !options.has(player.gameId)) options.set(player.gameId, player.name)
  }
  if (value && !options.has(value)) options.set(value, `Personnage ${value}`)
  return (
    <Field label="Joueur">
      {(props) => (
        <Select value={value ?? ALL} onValueChange={(next) => onChange(fromSelect(next))}>
          <SelectTrigger {...props}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Tous les joueurs</SelectItem>
            {[...options.entries()]
              .sort((a, b) => a[1].localeCompare(b[1], 'fr'))
              .map(([gameId, label]) => (
                <SelectItem key={gameId} value={gameId}>
                  {label}
                </SelectItem>
              ))}
          </SelectContent>
        </Select>
      )}
    </Field>
  )
}

function StockSection({
  guildId,
  propriete,
  canManage,
}: {
  guildId: string
  propriete: string | undefined
  canManage: boolean
}) {
  const stock = useQuery(coffreStockQuery(guildId, propriete))
  return (
    <Section
      title="Stock"
      description="Dépôts moins retraits lus depuis le début du suivi. Une quantité négative signale un stock présent avant le suivi."
    >
      {stock.isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-44 rounded-xl" />
          <Skeleton className="h-44 rounded-xl" />
        </div>
      ) : stock.isError ? (
        <QueryError error={stock.error} retry={() => void stock.refetch()} />
      ) : stock.data.properties.length === 0 ? (
        <EmptyState title="Aucun coffre suivi" icon={<PackageOpen />}>
          Les dépôts et retraits apparaissent dès que le bot lit les messages des salons de coffres.
        </EmptyState>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {stock.data.properties.map((property) => (
            <StockCard key={property.id} property={property} guildId={guildId} canManage={canManage} />
          ))}
        </div>
      )}
    </Section>
  )
}

function StockCard({
  property,
  guildId,
  canManage,
}: {
  property: CoffreStock['properties'][number]
  guildId: string
  canManage: boolean
}) {
  const lowCount = property.items.filter((item) => item.low).length
  return (
    <Card className="gap-3">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {propertyLabel(property)}
          {lowCount > 0 && (
            <Badge variant="warning">
              <TriangleAlert aria-hidden /> {lowCount} bas
            </Badge>
          )}
        </CardTitle>
        {canManage && (
          <CardAction>
            <RenamePropertyDialog guildId={guildId} property={property} />
          </CardAction>
        )}
      </CardHeader>
      <CardContent>
        {property.items.length === 0 ? (
          <p className="text-sm text-ink-muted">Aucun objet.</p>
        ) : (
          <ul className="divide-y text-sm">
            {property.items.map((item) => (
              <li key={item.item} className="flex items-center justify-between gap-3 py-1.5">
                <span className={cn('text-ink-muted', item.low && 'font-medium text-ink')}>{item.label}</span>
                <span className="flex items-center gap-2">
                  {item.low && item.quantity >= 0 && (
                    <Badge variant="warning" className="px-1.5">
                      Stock bas
                    </Badge>
                  )}
                  <span
                    className={cn(
                      'font-semibold tabular-nums',
                      item.quantity < 0 ? 'text-danger' : item.low ? 'text-internal' : 'text-ink',
                    )}
                    title={`Déposé ${formatNumber(item.deposited)}, retiré ${formatNumber(item.withdrawn)}`}
                  >
                    {formatNumber(item.quantity)}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}

function SeriesSection({
  guildId,
  objet,
  itemLabel,
  propriete,
  jours,
}: {
  guildId: string
  objet: string
  itemLabel: string
  propriete: string | undefined
  jours: string
}) {
  const series = useQuery(coffreSeriesQuery(guildId, objet, propriete, jours))
  return (
    <Section title={`Évolution : ${itemLabel}`} description="Stock cumulé et mouvements par jour sur la période.">
      {series.isPending ? (
        <Skeleton className="h-64 rounded-xl" />
      ) : series.isError ? (
        <QueryError error={series.error} retry={() => void series.refetch()} />
      ) : series.data.length === 0 ? (
        <EmptyState title="Aucune donnée sur la période" />
      ) : (
        <Card className={cn('pb-3', series.isPlaceholderData && 'opacity-60')}>
          <CardContent>
            <CoffreSeriesChart series={series.data} itemLabel={itemLabel} />
          </CardContent>
        </Card>
      )}
    </Section>
  )
}

function PlayersSection({
  guildId,
  filters,
  meta,
  canManage,
}: {
  guildId: string
  filters: CoffreFilters
  meta: CoffreMeta | undefined
  canManage: boolean
}) {
  const players = useQuery(coffrePlayersQuery(guildId, filters))
  return (
    <Section title="Par joueur" description="Activité de chaque personnage sur la sélection, et membre Discord relié.">
      {players.isPending ? (
        <TableSkeleton rows={4} />
      ) : players.isError ? (
        <QueryError error={players.error} retry={() => void players.refetch()} />
      ) : players.data.length === 0 ? (
        <EmptyState title="Aucun mouvement sur la sélection" />
      ) : (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Personnage</TableHead>
                <TableHead>Membre</TableHead>
                <TableHead className="text-right">Déposé</TableHead>
                <TableHead className="text-right">Retiré</TableHead>
                <TableHead className="text-right">Mouvements</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {players.data.map((player) => {
                const member =
                  player.member ?? meta?.links.find((link) => link.gameId === player.gameId)?.member ?? null
                return (
                  <TableRow key={player.gameId ?? player.name}>
                    <TableCell>
                      <span className="font-medium">{player.name}</span>
                      {player.gameId && <span className="ml-2 font-mono text-xs text-ink-subtle">{player.gameId}</span>}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1">
                        {member ? (
                          <MemberName member={member} guildId={guildId} />
                        ) : (
                          <span className="text-sm text-ink-subtle">Non relié</span>
                        )}
                        {canManage && player.gameId && (
                          <>
                            <LinkCharacterDialog
                              guildId={guildId}
                              gameId={player.gameId}
                              name={player.name}
                              current={member}
                            />
                            {member && (
                              <UnlinkCharacterButton
                                guildId={guildId}
                                gameId={player.gameId}
                                name={player.name}
                                member={member}
                              />
                            )}
                          </>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-right text-staff tabular-nums">
                      {formatNumber(player.deposited)}
                    </TableCell>
                    <TableCell className="text-right text-danger tabular-nums">
                      {formatNumber(player.withdrawn)}
                    </TableCell>
                    <TableCell className="text-right text-ink-muted tabular-nums">
                      {formatNumber(player.count)}
                    </TableCell>
                  </TableRow>
                )
              })}
            </TableBody>
          </Table>
        </Card>
      )}
    </Section>
  )
}

function MovementsSection({
  guildId,
  filters,
  page,
  onPage,
}: {
  guildId: string
  filters: CoffreFilters
  page: number
  onPage: (page: number) => void
}) {
  const movements = useQuery(coffreMovementsQuery(guildId, filters, page))
  return (
    <Section
      title="Mouvements"
      description={
        movements.data
          ? `${formatNumber(movements.data.total)} mouvement(s), du plus récent au plus ancien.`
          : undefined
      }
    >
      {movements.isPending ? (
        <TableSkeleton />
      ) : movements.isError ? (
        <QueryError error={movements.error} retry={() => void movements.refetch()} />
      ) : movements.data.items.length === 0 ? (
        <EmptyState title="Aucun mouvement sur la sélection" />
      ) : (
        <>
          <Card className={cn('py-0', movements.isPlaceholderData && 'opacity-60')}>
            <MovementsTable items={movements.data.items} guildId={guildId} />
          </Card>
          <Pagination
            page={movements.data.page}
            pageSize={movements.data.pageSize}
            total={movements.data.total}
            onPage={onPage}
            newerLabel="Plus récents"
            olderLabel="Plus anciens"
          />
        </>
      )}
    </Section>
  )
}
