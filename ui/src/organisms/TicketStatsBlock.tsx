import type { TicketStats } from '@/api/types'
import { Card } from '@/atoms/Card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatNumber } from '@/lib/format'
import { StatCard } from '@/molecules/StatCard'

/** Bloc « tickets » : chiffres sur 30 jours et tableau du staff (page staff et statistiques du serveur). */
export function TicketStatsBlock({ stats }: { stats: TicketStats }) {
  return (
    <>
      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
        <StatCard label="Ouverts" value={formatNumber(stats.open)} />
        <StatCard label="Créés (30 j)" value={formatNumber(stats.opened)} />
        <StatCard label="Fermés (30 j)" value={formatNumber(stats.closed)} />
        <StatCard
          label="Sans réponse"
          value={formatNumber(stats.unanswered)}
          tone={stats.unanswered > 0 ? 'danger' : 'default'}
        />
        <StatCard
          label="1re réponse (médiane)"
          value={stats.medianFirstResponse ?? '—'}
          tone="staff"
          className="col-span-2 sm:col-span-1"
        />
      </dl>
      {stats.staff.length === 0 ? (
        <p className="mt-4 text-sm text-ink-muted">Aucune réponse du staff ces 30 derniers jours.</p>
      ) : (
        <Card className="mt-4 py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Membre du staff</TableHead>
                <TableHead className="text-right">Réponses</TableHead>
                <TableHead className="text-right">Tickets</TableHead>
                <TableHead className="text-right">Pris en charge</TableHead>
                <TableHead className="text-right">1res réponses</TableHead>
                <TableHead className="text-right">Délai moyen</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {stats.staff.map((member) => (
                <TableRow key={member.name}>
                  <TableCell className="font-medium">{member.name}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatNumber(member.replies)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatNumber(member.tickets)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatNumber(member.claims)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatNumber(member.firstResponses)}</TableCell>
                  <TableCell className="text-right text-ink-muted tabular-nums">
                    {member.averageFirstResponse ?? '—'}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </>
  )
}
