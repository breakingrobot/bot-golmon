import type { CotisationEntry } from '@/api/types'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatDay, formatMoney } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { MemberName } from '@/molecules/MemberName'
import { DeleteCotisation } from '@/organisms/DeleteCotisation'

export function EntriesTable({ entries, showMember = true }: { entries: CotisationEntry[]; showMember?: boolean }) {
  const { guild, userId } = useGuild()
  const canManage = hasPerm(guild, 'cotisation.gerer')
  return (
    <Table>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead>Date</TableHead>
          {showMember && <TableHead>Membre</TableHead>}
          <TableHead>Description</TableHead>
          <TableHead className="text-right">Propre</TableHead>
          <TableHead className="text-right">Sale</TableHead>
          <TableHead className="w-10">
            <span className="sr-only">Actions</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {entries.map((entry) => {
          // Le serveur autorise l'auteur (le membre ou le gradé qui l'a déclarée) ou `cotisation.gerer`.
          const canDelete = canManage || entry.member.id === userId || entry.declaredBy?.id === userId
          return (
            <TableRow key={entry.id}>
              <TableCell className="text-ink-muted tabular-nums">{formatDay(entry.date)}</TableCell>
              {showMember && (
                <TableCell>
                  <MemberName member={entry.member} guildId={guild.id} />
                </TableCell>
              )}
              <TableCell className="max-w-80 text-sm whitespace-normal text-ink-muted">
                {entry.description || <span className="text-ink-subtle">—</span>}
                {entry.declaredBy && <span className="text-ink-subtle"> · déclarée par {entry.declaredBy.name}</span>}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {entry.propre ? formatMoney(entry.propre) : '—'}
              </TableCell>
              <TableCell className="text-right text-internal tabular-nums">
                {entry.sale ? formatMoney(entry.sale) : '—'}
              </TableCell>
              <TableCell className="text-right">{canDelete && <DeleteCotisation entry={entry} />}</TableCell>
            </TableRow>
          )
        })}
      </TableBody>
    </Table>
  )
}
