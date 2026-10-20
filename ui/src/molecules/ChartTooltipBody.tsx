import type { ReactNode } from 'react'

import type { Row, Series } from '@/lib/charts'

export type ChartTooltipBodyProps = {
  label: string
  row: Row
  series: Series[]
  formatValue: (value: number) => string
  footer?: ReactNode
}

/** Contenu de l'infobulle (l'enveloppe est celle de TanStack Charts, stylée par `.chart-tooltip`). */
export const ChartTooltipBody = ({ label, row, series, formatValue, footer }: ChartTooltipBodyProps) => (
  <div className="min-w-40 text-xs">
    <p className="mb-1 font-medium text-ink">{label}</p>
    <ul className="space-y-0.5">
      {series.map((item) => {
        const value = row[item.key]
        return (
          <li key={item.key} className="flex items-center justify-between gap-4">
            <span className="inline-flex items-center gap-1.5 text-ink-muted">
              <span className="size-2 rounded-full" style={{ background: item.color }} aria-hidden />
              {item.label}
            </span>
            <span className="font-medium text-ink tabular-nums">
              {typeof value === 'number' ? formatValue(value) : '—'}
            </span>
          </li>
        )
      })}
    </ul>
    {footer !== undefined && <div className="mt-1 border-t pt-1 text-ink-muted">{footer}</div>}
  </div>
)
