import type { CotisationEntry } from '@/api/types'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatDay, formatMoney } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { MemberName } from '@/molecules/MemberName'
import { MobileCard, MobileCards } from '@/molecules/MobileCards'
import { DeleteCotisation } from '@/organisms/DeleteCotisation'

export type EntriesTableProps = { entries: CotisationEntry[]; showMember?: boolean }

export const EntriesTable = ({ entries, showMember = true }: EntriesTableProps) => {
  const { guild, userId } = useGuild()
  const canManage = hasPerm(guild, 'cotisation.gerer')
  // Le serveur autorise l'auteur (le membre ou le gradé qui l'a déclarée) ou `cotisation.gerer`.
  const canDelete = (entry: CotisationEntry) =>
    canManage || entry.member.id === userId || entry.declaredBy?.id === userId
  return (
    <>
      <MobileCards label="Cotisations" flush>
        {entries.map((entry) => (
          <MobileCard
            key={entry.id}
            title={showMember ? <MemberName member={entry.member} guildId={guild.id} /> : formatDay(entry.date)}
            aside={canDelete(entry) && <DeleteCotisation entry={entry} />}
            fields={[
              ...(showMember ? [{ label: 'Date', value: formatDay(entry.date) }] : []),
              { label: 'Propre', value: entry.propre ? formatMoney(entry.propre) : '—' },
              { label: 'Sale', value: entry.sale ? formatMoney(entry.sale) : '—' },
            ]}
          >
            {(entry.description || entry.declaredBy) && (
              <p className="mt-1.5 text-ink-muted">
                {entry.description}
                {entry.declaredBy && <span className="text-ink-subtle"> · déclarée par {entry.declaredBy.name}</span>}
              </p>
            )}
          </MobileCard>
        ))}
      </MobileCards>
      <div className="hidden sm:block">
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
            {entries.map((entry) => (
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
                <TableCell className="text-right">{canDelete(entry) && <DeleteCotisation entry={entry} />}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </>
  )
}
