import { ClipboardList } from 'lucide-react'

import type { CoffreMovement } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { propertyLabel } from '@/lib/coffre'
import { formatDateTime, formatSignedNumber } from '@/lib/format'
import { cn } from '@/lib/utils'
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
            ) : movement.kind === 'WITHDRAW' ? (
              <Badge variant="danger">Retrait</Badge>
            ) : (
              <Badge variant="brand" title="Correction d’inventaire saisie par un gradé">
                <ClipboardList aria-hidden /> Correction
              </Badge>
            )}
          </TableCell>
          <TableCell className="font-medium">{movement.itemLabel}</TableCell>
          <TableCell className={cn('text-right tabular-nums', movement.kind === 'ADJUST' && 'font-medium text-brand')}>
            {/* Une correction porte déjà son signe ; dépôts et retraits sont des quantités positives. */}
            {formatSignedNumber(movement.kind === 'WITHDRAW' ? -movement.amount : movement.amount)}
          </TableCell>
          <TableCell>
            {movement.kind === 'ADJUST' ? (
              <span className="inline-flex items-center gap-2">
                {movement.player.member ? (
                  <MemberName member={movement.player.member} guildId={guildId} avatar={false} />
                ) : (
                  <span className="text-ink">{movement.player.name}</span>
                )}
                <span className="text-xs text-ink-muted">inventaire</span>
              </span>
            ) : (
              <span className="text-ink">{movement.player.name}</span>
            )}
            {movement.kind !== 'ADJUST' && movement.player.member && (
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
