import { crosshair, defineChart, lineY } from '@tanstack/charts'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { scalePoint } from '@tanstack/charts/scales/point'
import { Chart } from '@tanstack/react-charts/tooltip'

import { ATTENDANCE_SERIES, GRID, periodAxis, THEME, TICK_LABELS, TOOLTIP } from '@/lib/charts'
import { formatNumber, formatPercent, formatShortDay, formatWeek } from '@/lib/format'
import { ChartDataTable } from '@/molecules/ChartDataTable'
import { ChartTooltipBody } from '@/molecules/ChartTooltipBody'

export type AttendancePoint = { weekStart: string; rate: number | null; presences: number }

export type AttendanceChartProps = { data: AttendancePoint[] }

export const AttendanceChart = ({ data }: AttendanceChartProps) => {
  const series = ATTENDANCE_SERIES
  const definition = defineChart({
    marks: [
      crosshair({ x: { stroke: 'var(--line-strong)' }, y: false }),
      // Les semaines sans présence clôturée (taux nul) coupent la courbe.
      lineY(data, { x: 'weekStart', y: 'rate', stroke: 'var(--series-1)', strokeWidth: 2, points: true }),
    ],
    scales: {
      x: { scale: () => scalePoint<string>().padding(0.25), axis: periodAxis(formatShortDay) },
      y: {
        scale: scaleLinear().domain([0, 100]),
        grid: GRID,
        axis: {
          line: false,
          ticks: { size: 0, padding: 8, values: [0, 25, 50, 75, 100], format: (value: number) => `${value} %` },
          tickLabels: TICK_LABELS,
        },
      },
    },
    theme: THEME,
    focus: 'nearest-x',
    maxFocusDistance: Number.POSITIVE_INFINITY,
    tooltip: TOOLTIP,
  })
  return (
    <>
      <Chart
        definition={definition}
        height={240}
        className="chart"
        ariaLabel="Taux de présence moyen par semaine, sur les 12 dernières semaines"
        renderTooltipBody={({ points }) => {
          const datum = points[0]?.datum
          if (!datum) return null
          return (
            <ChartTooltipBody
              label={formatWeek(datum.weekStart)}
              row={datum}
              series={series}
              formatValue={formatPercent}
              footer={`${formatNumber(datum.presences)} présence(s) clôturée(s)`}
            />
          )
        }}
      />
      <ChartDataTable
        rows={data}
        xKey="weekStart"
        series={series}
        formatLabel={formatWeek}
        formatValue={formatPercent}
      />
    </>
  )
}
