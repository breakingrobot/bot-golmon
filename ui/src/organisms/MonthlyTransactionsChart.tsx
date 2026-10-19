import { barY, crosshair, defineChart, group } from '@tanstack/charts'
import { scaleBand } from '@tanstack/charts/scales/band'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { Chart } from '@tanstack/react-charts/tooltip'

import type { TransactionsResponse } from '@/api/types'
import { colorScale, GRID, periodAxis, THEME, toLong, TOOLTIP, TRANSACTION_SERIES, valueAxis } from '@/lib/charts'
import { formatCompactMoney, formatMoney, formatMonth, formatNumber } from '@/lib/format'
import { ChartDataTable } from '@/molecules/ChartDataTable'
import { ChartLegend } from '@/molecules/ChartLegend'
import { ChartTooltipBody } from '@/molecules/ChartTooltipBody'

export type MonthlyTransactionsChartProps = { months: TransactionsResponse['months'] }

export const MonthlyTransactionsChart = ({ months }: MonthlyTransactionsChartProps) => {
  const series = TRANSACTION_SERIES
  // L'API donne les mois du plus récent au plus ancien : ordre chronologique pour le graphique.
  const rows = months.toReversed()
  const long = toLong(rows, 'month', TRANSACTION_SERIES)
  const definition = defineChart({
    marks: [
      crosshair({ x: { band: { fill: 'var(--surface-sunken)', fillOpacity: 1 } }, y: false }),
      barY(long, {
        x: 'x',
        y: 'value',
        z: 'serie',
        color: 'serie',
        layout: group({ padding: 0.08 }),
        radius: [4, 4, 0, 0],
        maxThickness: 28,
      }),
    ],
    scales: {
      x: { scale: () => scaleBand<string>().padding(0.2), axis: periodAxis((month) => formatMonth(month)) },
      y: { scale: scaleLinear, nice: true, grid: GRID, axis: valueAxis(formatCompactMoney) },
    },
    color: colorScale(TRANSACTION_SERIES),
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
        height={240}
        className="chart"
        ariaLabel="Achats et ventes par mois"
        renderTooltipBody={({ points }) => {
          const row = points[0]?.datum.row
          if (!row) return null
          return (
            <ChartTooltipBody
              label={formatMonth(row.month, true)}
              row={row}
              series={series}
              formatValue={formatMoney}
              footer={`${formatNumber(row.count)} transaction(s)`}
            />
          )
        }}
      />
      <ChartDataTable
        rows={rows}
        xKey="month"
        series={series}
        formatLabel={(month) => formatMonth(month, true)}
        formatValue={formatMoney}
      />
    </>
  )
}
