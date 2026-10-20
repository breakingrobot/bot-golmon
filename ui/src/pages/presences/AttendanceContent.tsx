import { TriangleAlert } from 'lucide-react'

import type { PresenceStats } from '@/api/types'
import { Card, CardContent } from '@/atoms/Card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatNumber, formatPercent } from '@/lib/format'
import { attendanceTone } from '@/lib/tone'
import { EmptyState } from '@/molecules/EmptyState'
import { MemberName } from '@/molecules/MemberName'
import { Section } from '@/molecules/Section'
import { ShareBar } from '@/molecules/ShareBar'
import { StatCard } from '@/molecules/StatCard'
import { StatGrid } from '@/molecules/StatGrid'
import { AttendanceChart } from '@/organisms/AttendanceChart'

export type AttendanceContentProps = {
  stats: PresenceStats
  /** Lien vers la fiche des membres (`membre.fiche`). */
  linkTo: string | undefined
}

/** Chiffres, courbe hebdomadaire et classement de l'assiduité sur une période. */
export const AttendanceContent = ({ stats, linkTo }: AttendanceContentProps) => {
  const below = stats.members.filter((row) => row.rate !== null && row.rate < stats.threshold)
  return (
    <>
      <StatGrid className="mt-6 lg:grid-cols-3">
        <StatCard label="Présences closes" value={formatNumber(stats.presences)} hint="sur la période" />
        <StatCard
          label="Taux de présence"
          value={stats.rate === null ? '—' : formatPercent(stats.rate)}
          hint="présents ÷ attendus, retards à part"
          {...(stats.rate === null ? {} : { tone: attendanceTone(stats.rate) })}
        />
        <StatCard
          label="Sous le seuil"
          value={formatNumber(below.length)}
          hint={`moins de ${formatPercent(stats.threshold)}`}
          tone={below.length > 0 ? 'danger' : 'default'}
        />
      </StatGrid>

      <Section
        title="Assiduité par semaine"
        description="Taux de présence moyen, semaines vides comprises (règle du rapport hebdomadaire)."
      >
        <Card className="pb-3">
          <CardContent>
            <AttendanceChart data={stats.weekly} />
          </CardContent>
        </Card>
      </Section>

      <Section
        title="Classement"
        description="Présences closes où chaque membre était attendu : membres du rôle déjà arrivés, ou qui ont répondu."
      >
        {stats.members.length === 0 ? (
          <EmptyState title="Aucune présence close sur la période." />
        ) : (
          <>
            {/* Écran étroit : une carte par membre. */}
            <ol className="grid gap-2 sm:hidden" aria-label="Classement">
              {stats.members.map((row, index) => (
                <li key={row.member.id} className="rounded-xl border bg-card px-4 py-3 shadow-xs">
                  <div className="flex items-center gap-2">
                    <span className="w-6 text-sm text-ink-subtle tabular-nums">{index + 1}.</span>
                    <MemberName member={row.member} guildId={linkTo} className="min-w-0 flex-1" />
                    <span className="text-sm font-semibold tabular-nums">
                      {row.rate === null ? '—' : formatPercent(row.rate)}
                    </span>
                  </div>
                  <p className="mt-1 pl-8 text-xs text-ink-muted tabular-nums">
                    {formatNumber(row.present)}/{formatNumber(row.expected)} présent · {formatNumber(row.retard)} retard
                    · {formatNumber(row.absent)} absent · {formatNumber(row.noAnswer)} sans réponse
                  </p>
                </li>
              ))}
            </ol>
            <Card className="hidden py-0 sm:block">
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead className="w-10">#</TableHead>
                    <TableHead>Membre</TableHead>
                    <TableHead>Taux</TableHead>
                    <TableHead className="text-right">Présent</TableHead>
                    <TableHead className="text-right">Retard</TableHead>
                    <TableHead className="text-right">Absent</TableHead>
                    <TableHead className="text-right">Sans réponse</TableHead>
                    <TableHead className="text-right">Attendu</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {stats.members.map((row, index) => (
                    <TableRow key={row.member.id}>
                      <TableCell className="text-ink-subtle tabular-nums">{index + 1}</TableCell>
                      <TableCell>
                        <MemberName member={row.member} guildId={linkTo} />
                      </TableCell>
                      <TableCell>
                        {row.rate === null ? (
                          '—'
                        ) : (
                          <div className="flex items-center gap-2">
                            <ShareBar
                              value={row.rate}
                              label={`Taux de présence de ${row.member.name}`}
                              tone={attendanceTone(row.rate)}
                            />
                            <span className="text-ink-muted tabular-nums">{formatPercent(row.rate)}</span>
                            {row.rate < stats.threshold && (
                              <TriangleAlert className="size-4 text-danger" role="img" aria-label="Sous le seuil" />
                            )}
                          </div>
                        )}
                      </TableCell>
                      <TableCell className="text-right text-staff tabular-nums">{formatNumber(row.present)}</TableCell>
                      <TableCell className="text-right text-internal tabular-nums">
                        {formatNumber(row.retard)}
                      </TableCell>
                      <TableCell className="text-right text-danger tabular-nums">{formatNumber(row.absent)}</TableCell>
                      <TableCell className="text-right text-ink-muted tabular-nums">
                        {formatNumber(row.noAnswer)}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">{formatNumber(row.expected)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </Card>
          </>
        )}
      </Section>
    </>
  )
}
