import { TriangleAlert } from 'lucide-react'

import type { CoffreStock } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/atoms/Card'
import { propertyLabel } from '@/lib/coffre'
import { formatNumber } from '@/lib/format'
import { cn } from '@/lib/utils'
import { RenamePropertyDialog } from '@/organisms/RenamePropertyDialog'

export function StockCard({
  property,
  guildId,
  canManage,
}: {
  property: CoffreStock['properties'][number]
  guildId: string
  canManage: boolean
}) {
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
          <CardAction>
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
                    title={`Déposé ${formatNumber(item.deposited)}, retiré ${formatNumber(item.withdrawn)}`}
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
