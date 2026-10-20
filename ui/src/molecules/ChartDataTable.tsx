import type { Row, Series } from '@/lib/charts'

export type ChartDataTableProps = {
  rows: Row[]
  xKey: string
  series: Series[]
  formatLabel: (label: string) => string
  formatValue: (value: number) => string
}

/** Tableau des valeurs du graphique (accessibilité, lecture précise). */
export const ChartDataTable = ({ rows, xKey, series, formatLabel, formatValue }: ChartDataTableProps) => (
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
