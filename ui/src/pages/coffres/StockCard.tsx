import { ClipboardList, TriangleAlert } from 'lucide-react'

import type { CoffreStock } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/atoms/Card'
import { propertyLabel } from '@/lib/coffre'
import { formatNumber, formatSignedNumber } from '@/lib/format'
import { cn } from '@/lib/utils'
import { CoffreInventoryDialog } from '@/organisms/CoffreInventoryDialog'
import { RenamePropertyDialog } from '@/organisms/RenamePropertyDialog'

export type StockCardProps = {
  property: CoffreStock['properties'][number]
  guildId: string
  canManage: boolean
  /** Objets connus du serveur (suggestions de l'inventaire). */
  items: readonly { item: string; label: string }[]
}

export const StockCard = ({ property, guildId, canManage, items }: StockCardProps) => {
  const lowCount = property.items.filter((item) => item.low).length
  return (
    <Card className="gap-3">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {propertyLabel(property)}
          {lowCount > 0 && (
            <Badge variant="warning">
              <TriangleAlert aria-hidden /> {lowCount} bas
            </Badge>
          )}
        </CardTitle>
        {canManage && (
          <CardAction className="flex items-center gap-1">
            <CoffreInventoryDialog
              guildId={guildId}
              property={property}
              properties={[property]}
              items={items}
              trigger={
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={`Inventaire de ${propertyLabel(property)}`}
                  title="Inventaire"
                >
                  <ClipboardList aria-hidden />
                </Button>
              }
            />
            <RenamePropertyDialog guildId={guildId} property={property} />
          </CardAction>
        )}
      </CardHeader>
      <CardContent>
        {property.items.length === 0 ? (
          <p className="text-sm text-ink-muted">Aucun objet.</p>
        ) : (
          <ul className="divide-y text-sm">
            {property.items.map((item) => (
              <li key={item.item} className="flex items-center justify-between gap-3 py-1.5">
                <span className={cn('text-ink-muted', item.low && 'font-medium text-ink')}>{item.label}</span>
                <span className="flex items-center gap-2">
                  {item.adjusted !== 0 && (
                    <span
                      className="inline-flex items-center text-brand"
                      title={`dont ${formatSignedNumber(item.adjusted)} de corrections`}
                    >
                      <ClipboardList className="size-3.5" aria-hidden />
                      <span className="sr-only">dont {formatSignedNumber(item.adjusted)} de corrections</span>
                    </span>
                  )}
                  {item.low && item.quantity >= 0 && (
                    <Badge variant="warning" className="px-1.5">
                      Stock bas
                    </Badge>
                  )}
                  <span
                    className={cn(
                      'font-semibold tabular-nums',
                      item.quantity < 0 ? 'text-danger' : item.low ? 'text-internal' : 'text-ink',
                    )}
                    title={`Déposé ${formatNumber(item.deposited)}, retiré ${formatNumber(item.withdrawn)}${
                      item.adjusted === 0 ? '' : `, corrections ${formatSignedNumber(item.adjusted)}`
                    }`}
                  >
                    {formatNumber(item.quantity)}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}
