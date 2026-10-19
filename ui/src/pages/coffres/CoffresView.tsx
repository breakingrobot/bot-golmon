import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { X } from 'lucide-react'

import { csvUrl } from '@/api/client'
import { coffreMetaQuery } from '@/api/queries'
import type { CoffreFilters } from '@/api/queries'
import { Button } from '@/atoms/Button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { propertyLabel } from '@/lib/coffre'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect } from '@/lib/search'
import type { SearchPatch } from '@/lib/search'
import { CsvButton } from '@/molecules/CsvButton'
import { Field } from '@/molecules/Field'
import { PageHeader } from '@/molecules/PageHeader'
import { QueryError } from '@/molecules/QueryError'
import { PERIODS } from '@/pages/coffres/coffres-search'
import type { CoffresSearch, Period } from '@/pages/coffres/coffres-search'
import { MovementsSection } from '@/pages/coffres/MovementsSection'
import { PlayerSelect } from '@/pages/coffres/PlayerSelect'
import { PlayersSection } from '@/pages/coffres/PlayersSection'
import { SeriesSection } from '@/pages/coffres/SeriesSection'
import { StockSection } from '@/pages/coffres/StockSection'

const route = getRouteApi('/serveurs/$guildId/coffres')

export const CoffresView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/coffres' })
  const { page = 1, ...rest } = search
  const filters: CoffreFilters = { ...rest, jours: rest.jours ?? '30' }
  const meta = useQuery(coffreMetaQuery(guild.id))
  const setFilter = (patch: SearchPatch<CoffresSearch>) =>
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
