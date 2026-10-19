import { barY, crosshair, defineChart, stack } from '@tanstack/charts'
import { scaleBand } from '@tanstack/charts/scales/band'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { Chart } from '@tanstack/react-charts/tooltip'

import type { WeeklyAmount } from '@/api/types'
import { colorScale, COTISATION_SERIES, GRID, periodAxis, THEME, toLong, TOOLTIP, valueAxis } from '@/lib/charts'
import { formatCompactMoney, formatMoney, formatShortDay, formatWeek } from '@/lib/format'
import { ChartDataTable } from '@/molecules/ChartDataTable'
import { ChartLegend } from '@/molecules/ChartLegend'
import { ChartTooltipBody } from '@/molecules/ChartTooltipBody'

export type WeeklyCotisationsChartProps = { weekly: WeeklyAmount[]; height?: number }

export const WeeklyCotisationsChart = ({ weekly, height = 240 }: WeeklyCotisationsChartProps) => {
  const series = COTISATION_SERIES
  const long = toLong(weekly, 'weekStart', COTISATION_SERIES)
  const definition = defineChart({
    marks: [
      crosshair({ x: { band: { fill: 'var(--surface-sunken)', fillOpacity: 1 } }, y: false }),
      // Empilement : propre en bas, sale au-dessus ; seul le haut de la pile est arrondi.
      barY(long, {
        x: 'x',
        y: 'value',
        z: 'serie',
        color: 'serie',
        layout: stack({ order: ['propre', 'sale'] }),
        stroke: 'var(--surface)',
        strokeWidth: 1,
        radius: { end: 4 },
        maxThickness: 32,
      }),
    ],
    scales: {
      x: { scale: () => scaleBand<string>().padding(0.2), axis: periodAxis(formatShortDay) },
      y: { scale: scaleLinear, nice: true, grid: GRID, axis: valueAxis(formatCompactMoney) },
    },
    color: colorScale(COTISATION_SERIES),
    theme: THEME,
    focus: 'group-x',
    maxFocusDistance: Number.POSITIVE_INFINITY,
    // Le bandeau du curseur marque la période : pas d'anneau sur une barre (parfois nulle).
    focusRing: false,
    tooltip: { ...TOOLTIP, anchor: 'group-center' },
  })
  return (
    <>
      <ChartLegend series={series} />
      <Chart
        definition={definition}
        height={height}
        className="chart"
        ariaLabel="Cotisations par semaine, argent propre et sale empilés"
        renderTooltipBody={({ points }) => {
          const row = points[0]?.datum.row
          if (!row) return null
          return (
            <ChartTooltipBody
              label={formatWeek(row.weekStart)}
              row={row}
              series={series}
              formatValue={formatMoney}
              footer={`Total : ${formatMoney(row.propre + row.sale)}`}
            />
          )
        }}
      />
      <ChartDataTable
        rows={weekly}
        xKey="weekStart"
        series={series}
        formatLabel={formatWeek}
        formatValue={formatMoney}
      />
    </>
  )
}
