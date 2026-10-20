import { barY, crosshair, defineChart, group, lineY } from '@tanstack/charts'
import { scaleBand } from '@tanstack/charts/scales/band'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { scalePoint } from '@tanstack/charts/scales/point'
import { Chart } from '@tanstack/react-charts/tooltip'

import type { CoffreSeries } from '@/api/types'
import {
  ADJUST_SERIES,
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

export type CoffreSeriesChartProps = { series: CoffreSeries; itemLabel: string }

/**
 * Évolution d'un objet : stock cumulé (courbe, corrections comprises), puis dépôts, retraits et corrections
 * d'inventaire signées par jour (barres ; la série des corrections n'apparaît que si la période en a).
 */
export const CoffreSeriesChart = ({ series: points, itemLabel }: CoffreSeriesChartProps) => {
  const stock = STOCK_SERIES
  const moves = points.some((point) => point.adjusted !== 0) ? [...MOVE_SERIES, ...ADJUST_SERIES] : MOVE_SERIES
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
  const long = toLong(points, 'day', moves)
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
    color: colorScale(moves),
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
          ariaLabel={`Dépôts, retraits et corrections de ${itemLabel} par jour`}
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
