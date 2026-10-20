import type { ChartCurve, ChartGuideLineStyle, ChartTheme } from '@tanstack/charts'
import { tooltip } from '@tanstack/charts/tooltip'

export type Series = { key: string; label: string; color: string }

export type Row = Record<string, string | number | null>

/** Thème commun : textes et grille depuis les jetons du site. */
export const THEME = {
  foreground: 'var(--ink-subtle)',
  muted: 'var(--ink-subtle)',
  grid: 'var(--line)',
  background: 'transparent',
  palette: ['var(--series-1)', 'var(--series-2)', 'var(--series-3)'],
} satisfies Partial<ChartTheme>

export const GRID = { stroke: 'var(--line)', strokeOpacity: 1 } satisfies ChartGuideLineStyle

export const TICK_LABELS = { fontSize: 12 }

export const TOOLTIP = { use: tooltip, className: 'chart-tooltip' } as const

/** Axe des montants ou quantités : sans trait, graduations sans repère. */
export const valueAxis = (format: (value: number) => string) => ({
  line: false,
  ticks: { size: 0, padding: 8, format },
  tickLabels: TICK_LABELS,
})

/** Axe des périodes (semaines, mois, jours) : étiquettes éclaircies si elles se chevauchent. */
export const periodAxis = (format: (value: string) => string) => ({
  line: false,
  ticks: { size: 0, padding: 8, format },
  tickLabels: { ...TICK_LABELS, thin: { minGap: 12 } },
})

/** Courbe en escalier (valeur gardée jusqu'au point suivant), pour le stock cumulé. */
export const stepAfter: ChartCurve = {
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

/** Lignes « longues » (une par période et par série) pour les barres groupées ou empilées. */
export type LongRow<T> = { x: string; serie: string; value: number; row: T }

export const toLong = <T extends Record<string, unknown>>(
  rows: readonly T[],
  xKey: keyof T,
  series: Series[],
): LongRow<T>[] =>
  rows.flatMap((row) =>
    series.map((item) => {
      const value = row[item.key]
      return { x: String(row[xKey]), serie: item.key, value: typeof value === 'number' ? value : 0, row }
    }),
  )

export const colorScale = (series: Series[]) => ({
  domain: series.map((item) => item.key),
  range: series.map((item) => item.color),
})

export const ATTENDANCE_SERIES: Series[] = [{ key: 'rate', label: 'Taux de présence', color: 'var(--series-1)' }]

export const TRANSACTION_SERIES: Series[] = [
  { key: 'ventes', label: 'Ventes', color: 'var(--series-1)' },
  { key: 'achats', label: 'Achats', color: 'var(--series-2)' },
]

export const COTISATION_SERIES: Series[] = [
  { key: 'propre', label: 'Propre', color: 'var(--series-1)' },
  { key: 'sale', label: 'Sale', color: 'var(--series-2)' },
]

export const STOCK_SERIES: Series[] = [{ key: 'stock', label: 'Stock', color: 'var(--series-3)' }]

export const MOVE_SERIES: Series[] = [
  { key: 'deposited', label: 'Déposé', color: 'var(--series-1)' },
  { key: 'withdrawn', label: 'Retiré', color: 'var(--series-2)' },
]

/** Corrections d'inventaire (signées), ajoutées aux mouvements quand la période en contient. */
export const ADJUST_SERIES: Series[] = [{ key: 'adjusted', label: 'Corrigé', color: 'var(--series-3)' }]
