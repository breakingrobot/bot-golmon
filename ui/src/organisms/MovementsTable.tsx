import type { CoffreMovement } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { propertyLabel } from '@/lib/coffre'
import { formatDateTime, formatNumber } from '@/lib/format'
import { MemberName } from '@/molecules/MemberName'

export type MovementsTableProps = { items: CoffreMovement[]; guildId: string }

export const MovementsTable = ({ items, guildId }: MovementsTableProps) => (
  <Table>
    <TableHeader>
      <TableRow className="hover:bg-transparent">
        <TableHead>Date</TableHead>
        <TableHead>Type</TableHead>
        <TableHead>Objet</TableHead>
        <TableHead className="text-right">Quantité</TableHead>
        <TableHead>Personnage</TableHead>
        <TableHead>Propriété</TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      {items.map((movement) => (
        <TableRow key={movement.id}>
          <TableCell className="text-ink-muted tabular-nums">{formatDateTime(movement.at)}</TableCell>
          <TableCell>
            {movement.kind === 'DEPOSIT' ? (
              <Badge variant="success">Dépôt</Badge>
            ) : (
              <Badge variant="danger">Retrait</Badge>
            )}
          </TableCell>
          <TableCell className="font-medium">{movement.itemLabel}</TableCell>
          <TableCell className="text-right tabular-nums">
            {movement.kind === 'DEPOSIT' ? '+' : '−'}
            {formatNumber(movement.amount)}
          </TableCell>
          <TableCell>
            <span className="text-ink">{movement.player.name}</span>
            {movement.player.member && (
              <span className="ml-2 text-xs text-ink-muted">
                (
                <MemberName
                  member={movement.player.member}
                  guildId={guildId}
                  avatar={false}
                  className="font-normal text-ink-muted"
                />
                )
              </span>
            )}
          </TableCell>
          <TableCell className="text-ink-muted">
            {propertyLabel({ id: movement.propertyId, name: movement.propertyName })}
          </TableCell>
        </TableRow>
      ))}
    </TableBody>
  </Table>
)
