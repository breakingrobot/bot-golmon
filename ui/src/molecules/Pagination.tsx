import { ChevronLeft, ChevronRight } from 'lucide-react'

import { Button } from '@/atoms/Button'
import { formatNumber } from '@/lib/format'

export type PaginationProps = {
  page: number
  pageSize: number
  total: number
  onPage: (page: number) => void
  newerLabel?: string
  olderLabel?: string
}

export const Pagination = ({
  page,
  pageSize,
  total,
  onPage,
  newerLabel = 'Précédent',
  olderLabel = 'Suivant',
}: PaginationProps) => {
  const pages = Math.max(1, Math.ceil(total / Math.max(pageSize, 1)))
  if (pages <= 1) return null
  return (
    <nav className="mt-4 flex items-center justify-between gap-3 text-sm" aria-label="Pagination">
      <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => onPage(page - 1)}>
        <ChevronLeft aria-hidden /> {newerLabel}
      </Button>
      <span className="text-ink-muted tabular-nums">
        Page {formatNumber(page)} sur {formatNumber(pages)}
      </span>
      <Button variant="outline" size="sm" disabled={page >= pages} onClick={() => onPage(page + 1)}>
        {olderLabel} <ChevronRight aria-hidden />
      </Button>
    </nav>
  )
}
