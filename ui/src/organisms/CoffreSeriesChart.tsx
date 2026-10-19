import { barY, crosshair, defineChart, group, lineY } from '@tanstack/charts'
import { scaleBand } from '@tanstack/charts/scales/band'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { scalePoint } from '@tanstack/charts/scales/point'
import { Chart } from '@tanstack/react-charts/tooltip'

import type { CoffreSeries } from '@/api/types'
import {
  colorScale,
  GRID,
  MOVE_SERIES,
  periodAxis,
  stepAfter,
  STOCK_SERIES,
  THEME,
  toLong,
  TOOLTIP,
  valueAxis,
} from '@/lib/charts'
import { formatNumber, formatShortDay } from '@/lib/format'
import { ChartDataTable } from '@/molecules/ChartDataTable'
import { ChartLegend } from '@/molecules/ChartLegend'
import { ChartTooltipBody } from '@/molecules/ChartTooltipBody'

/** Évolution d'un objet : stock cumulé (courbe), puis dépôts et retraits par jour (barres). */
export function CoffreSeriesChart({ series: points, itemLabel }: { series: CoffreSeries; itemLabel: string }) {
  const stock = STOCK_SERIES
  const moves = MOVE_SERIES
  const stockDefinition = defineChart({
    marks: [
      crosshair({ x: { stroke: 'var(--line-strong)' }, y: false }),
      lineY(points, { x: 'day', y: 'stock', stroke: 'var(--series-3)', strokeWidth: 2, curve: stepAfter }),
    ],
    scales: {
      x: { scale: () => scalePoint<string>().padding(0.1), axis: periodAxis(formatShortDay) },
      y: { scale: scaleLinear, nice: true, grid: GRID, axis: valueAxis(formatNumber) },
    },
    theme: THEME,
    focus: 'nearest-x',
    maxFocusDistance: Number.POSITIVE_INFINITY,
    tooltip: TOOLTIP,
  })
  const long = toLong(points, 'day', MOVE_SERIES)
  const movesDefinition = defineChart({
    marks: [
      crosshair({ x: { band: { fill: 'var(--surface-sunken)', fillOpacity: 1 } }, y: false }),
      barY(long, {
        x: 'x',
        y: 'value',
        z: 'serie',
        color: 'serie',
        layout: group({ padding: 0.05 }),
        radius: [3, 3, 0, 0],
        maxThickness: 14,
      }),
    ],
    scales: {
      x: { scale: () => scaleBand<string>().padding(0.15), axis: periodAxis(formatShortDay) },
      y: { scale: scaleLinear, nice: true, grid: GRID, axis: valueAxis(formatNumber) },
    },
    color: colorScale(MOVE_SERIES),
    theme: THEME,
    focus: 'group-x',
    maxFocusDistance: Number.POSITIVE_INFINITY,
    // Le bandeau du curseur marque la période : pas d'anneau sur une barre (parfois nulle).
    focusRing: false,
    tooltip: { ...TOOLTIP, anchor: 'group-center' },
  })
  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <div className="min-w-0">
        <h3 className="mb-2 text-sm font-medium text-ink">Stock de {itemLabel}</h3>
        <Chart
          definition={stockDefinition}
          height={220}
          className="chart"
          ariaLabel={`Stock cumulé de ${itemLabel} par jour`}
          renderTooltipBody={({ points: focused }) => {
            const datum = focused[0]?.datum
            if (!datum) return null
            return (
              <ChartTooltipBody
                label={formatShortDay(datum.day)}
                row={datum}
                series={[...stock, ...moves]}
                formatValue={formatNumber}
              />
            )
          }}
        />
      </div>
      <div className="min-w-0">
        <h3 className="mb-2 text-sm font-medium text-ink">Mouvements par jour</h3>
        <ChartLegend series={moves} />
        <Chart
          definition={movesDefinition}
          height={194}
          className="chart"
          ariaLabel={`Dépôts et retraits de ${itemLabel} par jour`}
          renderTooltipBody={({ points: focused }) => {
            const row = focused[0]?.datum.row
            if (!row) return null
            return (
              <ChartTooltipBody label={formatShortDay(row.day)} row={row} series={moves} formatValue={formatNumber} />
            )
          }}
        />
      </div>
      <div className="lg:col-span-2">
        <ChartDataTable
          rows={points}
          xKey="day"
          series={[...moves, ...stock]}
          formatLabel={(value) => formatShortDay(value)}
          formatValue={formatNumber}
        />
      </div>
    </div>
  )
}
