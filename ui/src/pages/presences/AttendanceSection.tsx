import { useQuery } from '@tanstack/react-query'

import { presenceStatsQuery } from '@/api/queries-groupe'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { cn } from '@/lib/utils'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { AttendanceContent } from '@/pages/presences/AttendanceContent'
import { PERIODS } from '@/pages/presences/presences-search'
import type { Period } from '@/pages/presences/presences-search'

export type AttendanceSectionProps = { guildId: string; jours: Period; onPeriod: (jours: Period) => void }

export const AttendanceSection = ({ guildId, jours, onPeriod }: AttendanceSectionProps) => {
  const { guild } = useGuild()
  const query = useQuery(presenceStatsQuery(guildId, jours))
  const linkTo = hasPerm(guild, 'membre.fiche') ? guildId : undefined

  return (
    <>
      <ToggleGroup
        type="single"
        value={jours}
        onValueChange={(value) => value && onPeriod(value as Period)}
        aria-label="Période"
      >
        {PERIODS.map((period) => (
          <ToggleGroupItem key={period.value} value={period.value}>
            {period.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>

      {query.isPending ? (
        <div className="mt-6">
          <PageSkeleton stats={3} />
        </div>
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <div className={cn('transition-opacity', query.isPlaceholderData && 'opacity-60')}>
          <AttendanceContent stats={query.data} linkTo={linkTo} />
        </div>
      )}
    </>
  )
}
