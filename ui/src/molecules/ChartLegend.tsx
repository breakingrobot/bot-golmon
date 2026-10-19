import type { Series } from '@/lib/charts'

export type ChartLegendProps = { series: Series[] }

export const ChartLegend = ({ series }: ChartLegendProps) => {
  if (series.length < 2) return null
  return (
    <ul className="mb-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-muted" aria-label="Légende">
      {series.map((item) => (
        <li key={item.key} className="inline-flex items-center gap-1.5">
          <span className="size-2.5 rounded-[3px]" style={{ background: item.color }} aria-hidden />
          {item.label}
        </li>
      ))}
    </ul>
  )
}
