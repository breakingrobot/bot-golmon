// Export CSV côté client d'un tableau déjà chargé (ou de toutes ses pages, `rows` asynchrone), au
// format des exports du bot (src/lib/csv.ts). Pour une liste exportée par le bot lui-même (lien
// `…/export.csv`), garder `CsvButton`.

import { Download } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import { errorMessage } from '@/api/client'
import { Button } from '@/atoms/Button'
import { downloadCsv, toCsv } from '@/lib/csv'
import type { CsvColumn } from '@/lib/csv'
import { todayParis } from '@/lib/format'

export type CsvExportButtonProps<T> = {
  /** Début du nom du fichier : `contacts` → `contacts-2026-10-20.csv`. */
  filename: string
  columns: readonly CsvColumn<T>[]
  /** Lignes affichées, ou chargement de toutes les pages au clic. */
  rows: readonly T[] | (() => Promise<readonly T[]>)
  label?: string
  disabled?: boolean
}

/** Charge les lignes et propose le fichier ; une erreur de chargement part en toast. */
const exportCsv = async <T,>(props: Pick<CsvExportButtonProps<T>, 'filename' | 'columns' | 'rows'>) => {
  try {
    const list = typeof props.rows === 'function' ? await props.rows() : props.rows
    downloadCsv(`${props.filename}-${todayParis()}.csv`, toCsv(list, props.columns))
  } catch (error) {
    toast.error(errorMessage(error))
  }
}

export const CsvExportButton = <T,>({
  filename,
  columns,
  rows,
  label = 'Exporter en CSV',
  disabled = false,
}: CsvExportButtonProps<T>) => {
  const [busy, setBusy] = useState(false)
  const run = async () => {
    setBusy(true)
    await exportCsv({ filename, columns, rows })
    setBusy(false)
  }
  return (
    <Button variant="outline" size="sm" disabled={disabled || busy} onClick={() => void run()}>
      <Download aria-hidden /> {busy ? 'Export…' : label}
    </Button>
  )
}
