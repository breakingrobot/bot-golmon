import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'

import { csvUrl } from '@/api/client'
import { cotisationsQuery } from '@/api/queries'
import type { CotisationFilters } from '@/api/queries'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import { cn } from '@/lib/utils'
import { CsvButton } from '@/molecules/CsvButton'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { AddCotisationDialog } from '@/organisms/AddCotisationDialog'
import { PERIODS } from '@/pages/cotisations/cotisations-search'
import type { Period } from '@/pages/cotisations/cotisations-search'
import { CotisationsContent } from '@/pages/cotisations/CotisationsContent'
import { MemberFilter } from '@/pages/cotisations/MemberFilter'

const route = getRouteApi('/serveurs/$guildId/cotisations')

export const CotisationsView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/cotisations' })
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
            if (value) {
              void navigate({ search: (prev) => compact({ ...prev, jours: value as Period, page: undefined }) })
            }
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
