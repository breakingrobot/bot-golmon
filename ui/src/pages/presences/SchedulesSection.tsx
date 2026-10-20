import { useQuery } from '@tanstack/react-query'
import { CalendarClock } from 'lucide-react'

import { presenceSchedulesQuery } from '@/api/queries-groupe'
import { Card } from '@/atoms/Card'
import { formatDateTime } from '@/lib/format'
import { EmptyState } from '@/molecules/EmptyState'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { DeleteSchedule } from '@/pages/presences/DeleteSchedule'
import { NewScheduleDialog } from '@/pages/presences/NewScheduleDialog'

export type SchedulesSectionProps = { guildId: string }

export const SchedulesSection = ({ guildId }: SchedulesSectionProps) => {
  const query = useQuery(presenceSchedulesQuery(guildId))
  return (
    <Section
      title="Présences récurrentes"
      description="Publiées automatiquement chaque semaine, le nombre d'heures choisi avant l'événement."
      actions={<NewScheduleDialog guildId={guildId} />}
    >
      {query.isPending ? (
        <TableSkeleton />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : query.data.length === 0 ? (
        <EmptyState title="Aucune présence planifiée" icon={<CalendarClock />}>
          Planifiez une présence pour qu'elle soit publiée toute seule chaque semaine.
        </EmptyState>
      ) : (
        <Card className="gap-0 py-0">
          <ul className="divide-y">
            {query.data.map((schedule) => (
              <li key={schedule.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 sm:px-5">
                <span className="font-medium text-ink capitalize">
                  {schedule.weekdayLabel} à {schedule.time}
                </span>
                <span className="min-w-0 text-ink">{schedule.title}</span>
                <span className="text-sm text-ink-muted">publiée {schedule.leadHours} h avant</span>
                <span className="ml-auto flex items-center gap-2">
                  {schedule.nextEvent && (
                    <span className="text-xs text-ink-subtle">
                      Prochaine : <time dateTime={schedule.nextEvent}>{formatDateTime(schedule.nextEvent)}</time>
                    </span>
                  )}
                  <DeleteSchedule guildId={guildId} schedule={schedule} />
                </span>
                {schedule.description && (
                  <span className="w-full truncate text-sm text-ink-muted">{schedule.description}</span>
                )}
              </li>
            ))}
          </ul>
        </Card>
      )}
    </Section>
  )
}
