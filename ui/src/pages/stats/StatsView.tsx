import { useQuery } from '@tanstack/react-query'
import { CalendarOff, UserPlus } from 'lucide-react'

import { statsQuery } from '@/api/queries'
import { Badge } from '@/atoms/Badge'
import { Card, CardContent } from '@/atoms/Card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatDay, formatNumber, formatPercent } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { attendanceTone } from '@/lib/tone'
import { EmptyState } from '@/molecules/EmptyState'
import { MemberName } from '@/molecules/MemberName'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { ShareBar } from '@/molecules/ShareBar'
import { StatCard } from '@/molecules/StatCard'
import { StatGrid } from '@/molecules/StatGrid'
import { AttendanceChart } from '@/organisms/AttendanceChart'
import { TicketStatsBlock } from '@/organisms/TicketStatsBlock'

export function StatsView() {
  const { guild } = useGuild()
  const query = useQuery(statsQuery(guild.id))
  const canProfile = hasPerm(guild, 'membre.fiche')
  const linkTo = canProfile ? guild.id : undefined

  if (query.isPending) return <PageSkeleton stats={6} />
  if (query.isError) return <QueryError error={query.error} retry={() => void query.refetch()} />
  const stats = query.data

  return (
    <>
      <PageHeader title="Statistiques" description={guild.name} />
      <StatGrid className="lg:grid-cols-3 xl:grid-cols-6">
        {stats.figures.map((figure) => (
          <StatCard key={figure.key} label={figure.label} value={figure.value} hint={figure.hint} />
        ))}
      </StatGrid>

      <Section
        title="Assiduité aux présences"
        description="Taux moyen par semaine (12 dernières), puis par membre sur toutes les présences clôturées."
      >
        {stats.attendance.length === 0 ? (
          <EmptyState title="Aucune présence clôturée pour l'instant." />
        ) : (
          <>
            <Card className="pb-3">
              <CardContent>
                <AttendanceChart data={stats.attendanceWeekly} />
              </CardContent>
            </Card>
            <Card className="mt-4 py-0">
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead>Membre</TableHead>
                    <TableHead>Taux</TableHead>
                    <TableHead className="text-right">Présent</TableHead>
                    <TableHead className="text-right">Retard</TableHead>
                    <TableHead className="text-right">Absent</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {stats.attendance.map((row) => (
                    <TableRow key={row.member.id}>
                      <TableCell>
                        <MemberName member={row.member} {...(linkTo ? { guildId: linkTo } : {})} />
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-2">
                          <ShareBar
                            value={row.percent}
                            label={`Taux de présence de ${row.member.name}`}
                            tone={attendanceTone(row.percent)}
                          />
                          <span className="text-ink-muted tabular-nums">{formatPercent(row.percent)}</span>
                        </div>
                      </TableCell>
                      <TableCell className="text-right text-staff tabular-nums">{formatNumber(row.present)}</TableCell>
                      <TableCell className="text-right text-internal tabular-nums">
                        {formatNumber(row.retard)}
                      </TableCell>
                      <TableCell className="text-right text-danger tabular-nums">{formatNumber(row.absent)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </Card>
          </>
        )}
      </Section>

      <div className="mt-10 grid gap-x-6 gap-y-10 lg:grid-cols-2 [&>section]:mt-0">
        <Section title="Recrues de la semaine">
          {stats.recruits.length === 0 ? (
            <EmptyState title="Aucune recrue en période d'essai." icon={<UserPlus />} />
          ) : (
            <Card className="gap-0 py-0">
              <ul className="divide-y">
                {stats.recruits.map((recruit) => (
                  <li key={recruit.member.id} className="px-5 py-3">
                    <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
                      <MemberName
                        member={recruit.member}
                        {...(linkTo ? { guildId: linkTo } : {})}
                        className="min-w-0 basis-full sm:flex-1 sm:basis-auto"
                      />
                      <span className="text-sm text-staff tabular-nums" title="Pour">
                        Pour {recruit.pour}
                      </span>
                      <span className="text-sm text-danger tabular-nums" title="Contre">
                        Contre {recruit.contre}
                      </span>
                      <span className="text-sm text-ink-muted tabular-nums" title="Blanc">
                        Blanc {recruit.blanc}
                      </span>
                    </div>
                    {recruit.veto && (
                      <p className="mt-2 rounded-md bg-danger-soft px-2 py-1 text-xs text-danger">
                        Véto de {recruit.veto.by.name} · {recruit.veto.motif}
                      </p>
                    )}
                  </li>
                ))}
              </ul>
            </Card>
          )}
        </Section>

        <Section title="Absences en cours">
          {stats.absencesCurrent.length === 0 ? (
            <EmptyState title="Personne n'est absent en ce moment." icon={<CalendarOff />} />
          ) : (
            <Card className="gap-0 py-0">
              <ul className="divide-y">
                {stats.absencesCurrent.map((absence) => (
                  <li
                    key={`${absence.member.id}-${absence.from}`}
                    className="flex flex-wrap items-center gap-x-3 gap-y-1 px-5 py-3"
                  >
                    <MemberName
                      member={absence.member}
                      {...(linkTo ? { guildId: linkTo } : {})}
                      className="min-w-0 basis-full sm:flex-1 sm:basis-auto"
                    />
                    <Badge variant="warning">
                      {formatDay(absence.from)} → {formatDay(absence.to)}
                    </Badge>
                    {absence.motif && <span className="w-full text-sm text-ink-muted">{absence.motif}</span>}
                  </li>
                ))}
              </ul>
            </Card>
          )}
        </Section>
      </div>

      {stats.tickets && (
        <Section title="Tickets" description="30 derniers jours.">
          <TicketStatsBlock stats={stats.tickets} />
        </Section>
      )}
    </>
  )
}
