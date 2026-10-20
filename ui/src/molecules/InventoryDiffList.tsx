import { ArrowRight } from 'lucide-react'

import type { InventoryDiff } from '@/lib/coffre-inventory'
import { formatNumber, formatSignedNumber } from '@/lib/format'
import { cn } from '@/lib/utils'

export type InventoryDiffListProps = { diffs: readonly InventoryDiff[]; className?: string }

/** Écarts d'un inventaire : quantité connue → quantité comptée, écart signé (vert : ajout, rouge : retrait). */
export const InventoryDiffList = ({ diffs, className }: InventoryDiffListProps) => (
  <ul className={cn('min-w-0 divide-y text-sm', className)}>
    {diffs.map((diff) => (
      <li key={diff.item} className="flex items-center justify-between gap-3 py-1.5">
        <span className="min-w-0 truncate text-ink">{diff.label}</span>
        <span className="flex shrink-0 items-center gap-1.5 tabular-nums">
          <span className={cn('text-ink-subtle', diff.before < 0 && 'text-danger')}>{formatNumber(diff.before)}</span>
          <ArrowRight className="size-3.5 text-ink-subtle" aria-hidden />
          <span className="sr-only">devient</span>
          <span className="font-medium text-ink">{formatNumber(diff.after)}</span>
          <span
            className={cn(
              'min-w-12 rounded-full px-1.5 py-0.5 text-center text-xs font-semibold',
              diff.delta > 0 ? 'bg-staff-soft text-staff' : 'bg-danger-soft text-danger',
            )}
          >
            {formatSignedNumber(diff.delta)}
          </span>
        </span>
      </li>
    ))}
  </ul>
)
