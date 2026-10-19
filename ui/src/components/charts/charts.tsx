// Graphiques Recharts (chargés seulement par les pages qui en affichent).
//
// Règles : une seule échelle par graphique, couleurs catégorielles dans l'ordre fixe
// (--series-1, --series-2, validées clair/sombre), légende HTML dès deux séries,
// infobulle au survol, textes en couleurs de texte, tableau des données disponible.

import type { ReactNode } from 'react'
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
  type TooltipContentProps,
} from 'recharts'

import type { CoffreSeries, TransactionsResponse, WeeklyAmount } from '@/api/types'
import {
  formatCompactMoney,
  formatMoney,
  formatMonth,
  formatNumber,
  formatPercent,
  formatShortDay,
  formatWeek,
} from '@/lib/format'
import { cn } from '@/lib/utils'

type Series = { key: string; label: string; color: string }

const AXIS_TICK = { fill: 'var(--ink-subtle)', fontSize: 12 }
const GRID_STROKE = 'var(--line)'

function Legend({ series }: { series: Series[] }) {
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

type Row = Record<string, string | number | null>

function ChartTooltip({
  active,
  payload,
  label,
  series,
  formatLabel,
  formatValue,
  footer,
}: {
  active: boolean | undefined
  payload: readonly { dataKey?: unknown; value?: unknown; payload?: unknown }[] | undefined
  label: unknown
  series: Series[]
  formatLabel: (label: string) => string
  formatValue: (value: number) => string
  footer?: ((row: Row) => ReactNode) | undefined
}) {
  if (!active || !payload?.length) return null
  const row = (payload[0]?.payload ?? {}) as Row
  return (
    <div className="min-w-40 rounded-lg border bg-popover px-3 py-2 text-xs shadow-md">
      <p className="mb-1 font-medium text-ink">{formatLabel(String(label ?? ''))}</p>
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
      {footer && <div className="mt-1 border-t pt-1 text-ink-muted">{footer(row)}</div>}
    </div>
  )
}

/** Tableau des valeurs du graphique (accessibilité, lecture précise). */
function DataTable({
  rows,
  xKey,
  series,
  formatLabel,
  formatValue,
}: {
  rows: Row[]
  xKey: string
  series: Series[]
  formatLabel: (label: string) => string
  formatValue: (value: number) => string
}) {
  return (
    <details className="group mt-2 text-sm">
      <summary className="inline-flex cursor-pointer rounded-sm text-xs font-medium text-ink-muted hover:text-ink">
        Afficher les données
      </summary>
      <div className="mt-2 max-h-64 overflow-auto rounded-lg border">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-muted text-ink-subtle">
            <tr>
              <th className="px-3 py-1.5 text-left font-medium">Période</th>
              {series.map((item) => (
                <th key={item.key} className="px-3 py-1.5 text-right font-medium">
                  {item.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y">
            {rows.map((row) => (
              <tr key={String(row[xKey])}>
                <td className="px-3 py-1.5">{formatLabel(String(row[xKey]))}</td>
                {series.map((item) => {
                  const value = row[item.key]
                  return (
                    <td key={item.key} className="px-3 py-1.5 text-right tabular-nums">
                      {typeof value === 'number' ? formatValue(value) : '—'}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </details>
  )
}

function ChartFrame({
  label,
  height = 240,
  children,
  className,
}: {
  label: string
  height?: number
  children: ReactNode
  className?: string
}) {
  return (
    <div role="img" aria-label={label} className={cn('w-full', className)} style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        {children}
      </ResponsiveContainer>
    </div>
  )
}

function tooltipContent(
  series: Series[],
  formatLabel: (label: string) => string,
  formatValue: (value: number) => string,
  footer?: (row: Row) => ReactNode,
) {
  return function Content(props: TooltipContentProps) {
    return (
      <ChartTooltip
        active={props.active}
        payload={props.payload}
        label={props.label}
        series={series}
        formatLabel={formatLabel}
        formatValue={formatValue}
        footer={footer}
      />
    )
  }
}

// ---------------------------------------------------------------------------

export function AttendanceChart({ data }: { data: { weekStart: string; rate: number | null; presences: number }[] }) {
  const series: Series[] = [{ key: 'rate', label: 'Taux de présence', color: 'var(--series-1)' }]
  const rows = data as unknown as Row[]
  return (
    <>
      <ChartFrame label="Taux de présence moyen par semaine, sur les 12 dernières semaines">
        <LineChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} stroke={GRID_STROKE} />
          <XAxis
            dataKey="weekStart"
            tickFormatter={formatShortDay}
            tick={AXIS_TICK}
            tickLine={false}
            axisLine={false}
            minTickGap={16}
          />
          <YAxis
            domain={[0, 100]}
            ticks={[0, 25, 50, 75, 100]}
            tickFormatter={(value: number) => `${value} %`}
            tick={AXIS_TICK}
            tickLine={false}
            axisLine={false}
            width={52}
          />
          <Tooltip
            cursor={{ stroke: 'var(--line-strong)' }}
            content={tooltipContent(
              series,
              formatWeek,
              formatPercent,
              (row) => `${formatNumber(Number(row.presences ?? 0))} présence(s) clôturée(s)`,
            )}
          />
          <Line
            type="monotone"
            dataKey="rate"
            stroke="var(--series-1)"
            strokeWidth={2}
            dot={{ r: 3, fill: 'var(--series-1)', stroke: 'var(--surface)', strokeWidth: 2 }}
            activeDot={{ r: 5, stroke: 'var(--surface)', strokeWidth: 2 }}
            connectNulls={false}
            isAnimationActive={false}
          />
        </LineChart>
      </ChartFrame>
      <DataTable rows={rows} xKey="weekStart" series={series} formatLabel={formatWeek} formatValue={formatPercent} />
    </>
  )
}

export function MonthlyTransactionsChart({ months }: { months: TransactionsResponse['months'] }) {
  const series: Series[] = [
    { key: 'ventes', label: 'Ventes', color: 'var(--series-1)' },
    { key: 'achats', label: 'Achats', color: 'var(--series-2)' },
  ]
  // L'API donne les mois du plus récent au plus ancien : ordre chronologique pour le graphique.
  const rows = months.toReversed() as unknown as Row[]
  const money = (value: number) => formatMoney(value)
  return (
    <>
      <Legend series={series} />
      <ChartFrame label="Achats et ventes par mois">
        <BarChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }} barGap={2}>
          <CartesianGrid vertical={false} stroke={GRID_STROKE} />
          <XAxis
            dataKey="month"
            tickFormatter={(month: string) => formatMonth(month)}
            tick={AXIS_TICK}
            tickLine={false}
            axisLine={false}
          />
          <YAxis tickFormatter={formatCompactMoney} tick={AXIS_TICK} tickLine={false} axisLine={false} width={64} />
          <Tooltip
            cursor={{ fill: 'var(--surface-sunken)' }}
            content={tooltipContent(
              series,
              (month) => formatMonth(month, true),
              money,
              (row) => `${formatNumber(Number(row.count ?? 0))} transaction(s)`,
            )}
          />
          {series.map((item) => (
            <Bar
              key={item.key}
              dataKey={item.key}
              name={item.label}
              fill={item.color}
              radius={[4, 4, 0, 0]}
              maxBarSize={28}
              isAnimationActive={false}
            />
          ))}
        </BarChart>
      </ChartFrame>
      <DataTable
        rows={rows}
        xKey="month"
        series={series}
        formatLabel={(month) => formatMonth(month, true)}
        formatValue={money}
      />
    </>
  )
}

export function WeeklyCotisationsChart({ weekly, height = 240 }: { weekly: WeeklyAmount[]; height?: number }) {
  const series: Series[] = [
    { key: 'propre', label: 'Propre', color: 'var(--series-1)' },
    { key: 'sale', label: 'Sale', color: 'var(--series-2)' },
  ]
  const rows = weekly.map((week) => ({ ...week, total: week.propre + week.sale })) as unknown as Row[]
  const money = (value: number) => formatMoney(value)
  return (
    <>
      <Legend series={series} />
      <ChartFrame label="Cotisations par semaine, argent propre et sale empilés" height={height}>
        <BarChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} stroke={GRID_STROKE} />
          <XAxis
            dataKey="weekStart"
            tickFormatter={formatShortDay}
            tick={AXIS_TICK}
            tickLine={false}
            axisLine={false}
            minTickGap={12}
          />
          <YAxis tickFormatter={formatCompactMoney} tick={AXIS_TICK} tickLine={false} axisLine={false} width={64} />
          <Tooltip
            cursor={{ fill: 'var(--surface-sunken)' }}
            content={tooltipContent(series, formatWeek, money, (row) => `Total : ${money(Number(row.total ?? 0))}`)}
          />
          <Bar
            dataKey="propre"
            stackId="a"
            fill="var(--series-1)"
            stroke="var(--surface)"
            strokeWidth={1}
            maxBarSize={32}
            isAnimationActive={false}
          />
          <Bar
            dataKey="sale"
            stackId="a"
            fill="var(--series-2)"
            stroke="var(--surface)"
            strokeWidth={1}
            radius={[4, 4, 0, 0]}
            maxBarSize={32}
            isAnimationActive={false}
          />
        </BarChart>
      </ChartFrame>
      <DataTable rows={rows} xKey="weekStart" series={series} formatLabel={formatWeek} formatValue={money} />
    </>
  )
}

/** Évolution d'un objet : stock cumulé (courbe), puis dépôts et retraits par jour (barres). */
export function CoffreSeriesChart({ series: points, itemLabel }: { series: CoffreSeries; itemLabel: string }) {
  const stock: Series[] = [{ key: 'stock', label: 'Stock', color: 'var(--series-3)' }]
  const moves: Series[] = [
    { key: 'deposited', label: 'Déposé', color: 'var(--series-1)' },
    { key: 'withdrawn', label: 'Retiré', color: 'var(--series-2)' },
  ]
  const rows = points as unknown as Row[]
  const day = (value: string) => formatShortDay(value)
  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <div>
        <h3 className="mb-2 text-sm font-medium text-ink">Stock de {itemLabel}</h3>
        <ChartFrame label={`Stock cumulé de ${itemLabel} par jour`} height={220}>
          <LineChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
            <CartesianGrid vertical={false} stroke={GRID_STROKE} />
            <XAxis
              dataKey="day"
              tickFormatter={day}
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={false}
              minTickGap={24}
            />
            <YAxis tickFormatter={formatNumber} tick={AXIS_TICK} tickLine={false} axisLine={false} width={48} />
            <Tooltip
              cursor={{ stroke: 'var(--line-strong)' }}
              content={tooltipContent([...stock, ...moves], day, formatNumber)}
            />
            <Line
              type="stepAfter"
              dataKey="stock"
              stroke="var(--series-3)"
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 5, stroke: 'var(--surface)', strokeWidth: 2 }}
              isAnimationActive={false}
            />
          </LineChart>
        </ChartFrame>
      </div>
      <div>
        <h3 className="mb-2 text-sm font-medium text-ink">Mouvements par jour</h3>
        <Legend series={moves} />
        <ChartFrame label={`Dépôts et retraits de ${itemLabel} par jour`} height={194}>
          <BarChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }} barGap={1}>
            <CartesianGrid vertical={false} stroke={GRID_STROKE} />
            <XAxis
              dataKey="day"
              tickFormatter={day}
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={false}
              minTickGap={24}
            />
            <YAxis tickFormatter={formatNumber} tick={AXIS_TICK} tickLine={false} axisLine={false} width={48} />
            <Tooltip cursor={{ fill: 'var(--surface-sunken)' }} content={tooltipContent(moves, day, formatNumber)} />
            {moves.map((item) => (
              <Bar
                key={item.key}
                dataKey={item.key}
                fill={item.color}
                radius={[3, 3, 0, 0]}
                maxBarSize={14}
                isAnimationActive={false}
              />
            ))}
          </BarChart>
        </ChartFrame>
      </div>
      <div className="lg:col-span-2">
        <DataTable
          rows={rows}
          xKey="day"
          series={[...moves, ...stock]}
          formatLabel={(value) => formatShortDay(value)}
          formatValue={formatNumber}
        />
      </div>
    </div>
  )
}
