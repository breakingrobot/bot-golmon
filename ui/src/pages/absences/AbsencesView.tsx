import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { CalendarOff } from 'lucide-react'
import { useState } from 'react'

import { absencesQuery } from '@/api/queries-groupe'
import type { AbsenceEntry } from '@/api/types'
import { Card } from '@/atoms/Card'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { nowIso, timelineDays } from '@/lib/absence-timeline'
import { formatNumber, todayParis } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { compact } from '@/lib/search'
import { EmptyState } from '@/molecules/EmptyState'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { AbsenceList } from '@/pages/absences/AbsenceList'
import { VIEWS } from '@/pages/absences/absences-search'
import type { View } from '@/pages/absences/absences-search'
import { AbsenceTimeline } from '@/pages/absences/AbsenceTimeline'
import { DeclareAbsenceDialog } from '@/pages/absences/DeclareAbsenceDialog'

const route = getRouteApi('/serveurs/$guildId/absences')

export const AbsencesView = () => {
  const { guild, userId } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/absences' })
  const query = useQuery(absencesQuery(guild.id))
  const now = nowIso()
  const today = todayParis()
  const [selected, setSelected] = useState<AbsenceEntry | null>(null)
  const view: View = search.vue ?? 'frise'
  const canManage = hasPerm(guild, 'absence.autrui')
  const linkTo = hasPerm(guild, 'membre.fiche') ? guild.id : undefined
  const listProps = { guildId: guild.id, linkTo, userId, canEndOthers: canManage, now }

  return (
    <>
      <PageHeader
        title="Absences"
        description={`${guild.name} · en cours, à venir et terminées depuis 30 jours`}
        actions={guild.inGroup && <DeclareAbsenceDialog guildId={guild.id} canManage={canManage} />}
      />

      <ToggleGroup
        type="single"
        value={view}
        onValueChange={(value) => {
          if (value) void navigate({ search: compact({ vue: value === 'frise' ? undefined : (value as View) }) })
        }}
        aria-label="Vue"
        className="mb-4"
      >
        {VIEWS.map((item) => (
          <ToggleGroupItem key={item.value} value={item.value}>
            {item.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>

      {query.isPending ? (
        <PageSkeleton stats={0} />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <>
          {view === 'frise' && (
            <>
              {/* Écran large : frise ; écran étroit : la liste des absences d'actualité. */}
              <Card className="hidden gap-0 overflow-x-auto py-0 md:block">
                <div className="min-w-[44rem]">
                  <AbsenceTimeline
                    absences={[...query.data.current, ...query.data.upcoming, ...query.data.recent]}
                    days={timelineDays(today)}
                    today={today}
                    now={now}
                    selectedId={selected?.id ?? null}
                    onSelect={setSelected}
                  />
                </div>
              </Card>
              <p className="mt-2 hidden flex-wrap gap-4 text-xs text-ink-muted md:flex">
                <span className="inline-flex items-center gap-1.5">
                  <span className="size-3 rounded-sm bg-internal" aria-hidden /> En cours
                </span>
                <span className="inline-flex items-center gap-1.5">
                  <span className="size-3 rounded-sm bg-brand" aria-hidden /> À venir
                </span>
                <span className="inline-flex items-center gap-1.5">
                  <span className="size-3 rounded-sm bg-muted" aria-hidden /> Terminée
                </span>
                <span>Flèches du clavier pour passer d'une absence à l'autre, Entrée pour le détail.</span>
              </p>
              {selected && (
                <div className="mt-4 hidden md:block">
                  <AbsenceList absences={[selected]} {...listProps} />
                </div>
              )}
            </>
          )}
          <div className={view === 'frise' ? 'md:hidden' : undefined}>
            <Section title="En cours" description={`${formatNumber(query.data.current.length)} absence(s)`}>
              {query.data.current.length === 0 ? (
                <EmptyState title="Personne n'est absent en ce moment." icon={<CalendarOff />} />
              ) : (
                <AbsenceList absences={query.data.current} {...listProps} />
              )}
            </Section>
            <Section title="À venir">
              {query.data.upcoming.length === 0 ? (
                <EmptyState title="Aucune absence prévue." />
              ) : (
                <AbsenceList absences={query.data.upcoming} {...listProps} />
              )}
            </Section>
            {view === 'liste' && (
              <Section title="Récentes" description="Terminées ou annulées depuis 30 jours.">
                {query.data.recent.length === 0 ? (
                  <EmptyState title="Aucune absence récente." />
                ) : (
                  <AbsenceList absences={query.data.recent} {...listProps} />
                )}
              </Section>
            )}
          </div>
        </>
      )}
    </>
  )
}
