// Graphiques TanStack Charts (chargés seulement par les pages qui en affichent).
//
// Règles : une seule échelle par graphique, couleurs catégorielles dans l'ordre fixe
// (--series-1, --series-2, validées clair/sombre), légende HTML dès deux séries,
// infobulle au survol et au clavier, textes en couleurs de texte, tableau des données disponible.
// Les couleurs sont des références aux jetons CSS (`var(--…)`) : le passage clair/sombre
// ne reconstruit pas le graphique. Les définitions ne sont pas enveloppées dans `useMemo` :
// le React Compiler les mémoïse selon les données reçues (identité stable = pas de reconstruction).

import { barY, crosshair, defineChart, group, lineY, stack, type ChartCurve } from '@tanstack/charts'
import { scaleBand } from '@tanstack/charts/scales/band'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { scalePoint } from '@tanstack/charts/scales/point'
import { tooltip } from '@tanstack/charts/tooltip'
import { Chart } from '@tanstack/react-charts/tooltip'
import type { ReactNode } from 'react'

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

type Series = { key: string; label: string; color: string }
type Row = Record<string, string | number | null>

/** Thème commun : textes et grille depuis les jetons du site. */
const THEME = {
  foreground: 'var(--ink-subtle)',
  muted: 'var(--ink-subtle)',
  grid: 'var(--line)',
  background: 'transparent',
  palette: ['var(--series-1)', 'var(--series-2)', 'var(--series-3)'],
}
const GRID = { stroke: 'var(--line)', strokeOpacity: 1 }
const TICK_LABELS = { fontSize: 12 }
const TOOLTIP = { use: tooltip, className: 'chart-tooltip' } as const

/** Axe des montants ou quantités : sans trait, graduations sans repère. */
const valueAxis = (format: (value: number) => string) => ({
  line: false,
  ticks: { size: 0, padding: 8, format },
  tickLabels: TICK_LABELS,
})

/** Axe des périodes (semaines, mois, jours) : étiquettes éclaircies si elles se chevauchent. */
const periodAxis = (format: (value: string) => string) => ({
  line: false,
  ticks: { size: 0, padding: 8, format },
  tickLabels: { ...TICK_LABELS, thin: { minGap: 12 } },
})

/** Courbe en escalier (valeur gardée jusqu'au point suivant), pour le stock cumulé. */
const stepAfter: ChartCurve = {
  line: (points) => points.map(([x, y], index) => (index === 0 ? `M${x},${y}` : `H${x}V${y}`)).join(''),
  area: (top, bottom) => {
    const upper = top.map(([x, y], index) => (index === 0 ? `M${x},${y}` : `H${x}V${y}`)).join('')
    const lower = bottom
      .toReversed()
      .map(([x, y]) => `L${x},${y}`)
      .join('')
    return `${upper}${lower}Z`
  },
}

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

/** Contenu de l'infobulle (l'enveloppe est celle de TanStack Charts, stylée par `.chart-tooltip`). */
function TooltipBody({
  label,
  row,
  series,
  formatValue,
  footer,
}: {
  label: string
  row: Row
  series: Series[]
  formatValue: (value: number) => string
  footer?: ReactNode
}) {
  return (
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

/** Lignes « longues » (une par période et par série) pour les barres groupées ou empilées. */
type LongRow<T> = { x: string; serie: string; value: number; row: T }

function toLong<T extends Record<string, unknown>>(rows: readonly T[], xKey: keyof T, series: Series[]): LongRow<T>[] {
  return rows.flatMap((row) =>
    series.map((item) => {
      const value = row[item.key]
      return { x: String(row[xKey]), serie: item.key, value: typeof value === 'number' ? value : 0, row }
    }),
  )
}

const colorScale = (series: Series[]) => ({
  domain: series.map((item) => item.key),
  range: series.map((item) => item.color),
})

const ATTENDANCE_SERIES: Series[] = [{ key: 'rate', label: 'Taux de présence', color: 'var(--series-1)' }]
const TRANSACTION_SERIES: Series[] = [
  { key: 'ventes', label: 'Ventes', color: 'var(--series-1)' },
  { key: 'achats', label: 'Achats', color: 'var(--series-2)' },
]
const COTISATION_SERIES: Series[] = [
  { key: 'propre', label: 'Propre', color: 'var(--series-1)' },
  { key: 'sale', label: 'Sale', color: 'var(--series-2)' },
]
const STOCK_SERIES: Series[] = [{ key: 'stock', label: 'Stock', color: 'var(--series-3)' }]
const MOVE_SERIES: Series[] = [
  { key: 'deposited', label: 'Déposé', color: 'var(--series-1)' },
  { key: 'withdrawn', label: 'Retiré', color: 'var(--series-2)' },
]

// ---------------------------------------------------------------------------

type AttendancePoint = { weekStart: string; rate: number | null; presences: number }

export function AttendanceChart({ data }: { data: AttendancePoint[] }) {
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
            <TooltipBody
              label={formatWeek(datum.weekStart)}
              row={datum}
              series={series}
              formatValue={formatPercent}
              footer={`${formatNumber(datum.presences)} présence(s) clôturée(s)`}
            />
          )
        }}
      />
      <DataTable rows={data} xKey="weekStart" series={series} formatLabel={formatWeek} formatValue={formatPercent} />
    </>
  )
}

export function MonthlyTransactionsChart({ months }: { months: TransactionsResponse['months'] }) {
  const series = TRANSACTION_SERIES
  // L'API donne les mois du plus récent au plus ancien : ordre chronologique pour le graphique.
  const rows = months.toReversed()
  const money = (value: number) => formatMoney(value)
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
      <Legend series={series} />
      <Chart
        definition={definition}
        height={240}
        className="chart"
        ariaLabel="Achats et ventes par mois"
        renderTooltipBody={({ points }) => {
          const row = points[0]?.datum.row
          if (!row) return null
          return (
            <TooltipBody
              label={formatMonth(row.month, true)}
              row={row}
              series={series}
              formatValue={money}
              footer={`${formatNumber(row.count)} transaction(s)`}
            />
          )
        }}
      />
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
  const series = COTISATION_SERIES
  const money = (value: number) => formatMoney(value)
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
      <Legend series={series} />
      <Chart
        definition={definition}
        height={height}
        className="chart"
        ariaLabel="Cotisations par semaine, argent propre et sale empilés"
        renderTooltipBody={({ points }) => {
          const row = points[0]?.datum.row
          if (!row) return null
          return (
            <TooltipBody
              label={formatWeek(row.weekStart)}
              row={row}
              series={series}
              formatValue={money}
              footer={`Total : ${money(row.propre + row.sale)}`}
            />
          )
        }}
      />
      <DataTable rows={weekly} xKey="weekStart" series={series} formatLabel={formatWeek} formatValue={money} />
    </>
  )
}

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
              <TooltipBody
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
        <Legend series={moves} />
        <Chart
          definition={movesDefinition}
          height={194}
          className="chart"
          ariaLabel={`Dépôts et retraits de ${itemLabel} par jour`}
          renderTooltipBody={({ points: focused }) => {
            const row = focused[0]?.datum.row
            if (!row) return null
            return <TooltipBody label={formatShortDay(row.day)} row={row} series={moves} formatValue={formatNumber} />
          }}
        />
      </div>
      <div className="lg:col-span-2">
        <DataTable
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
