import type { ComponentProps } from 'react'

import { cn } from '@/lib/utils'

export const Table = ({ className, ...props }: ComponentProps<'table'>) => (
  <div data-slot="table-container" className="relative w-full overflow-x-auto">
    <table data-slot="table" className={cn('w-full caption-bottom text-sm', className)} {...props} />
  </div>
)

export const TableHeader = ({ className, ...props }: ComponentProps<'thead'>) => (
  <thead data-slot="table-header" className={cn('[&_tr]:border-b', className)} {...props} />
)

export const TableBody = ({ className, ...props }: ComponentProps<'tbody'>) => (
  <tbody data-slot="table-body" className={cn('[&_tr:last-child]:border-0', className)} {...props} />
)

export const TableRow = ({ className, ...props }: ComponentProps<'tr'>) => (
  <tr
    data-slot="table-row"
    className={cn('border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted', className)}
    {...props}
  />
)

export const TableHead = ({ className, ...props }: ComponentProps<'th'>) => (
  <th
    data-slot="table-head"
    className={cn(
      'h-9 px-3 text-left align-middle text-xs font-medium tracking-wide whitespace-nowrap text-ink-subtle uppercase first:pl-5 last:pr-5',
      className,
    )}
    {...props}
  />
)

export const TableCell = ({ className, ...props }: ComponentProps<'td'>) => (
  <td
    data-slot="table-cell"
    className={cn('px-3 py-2.5 align-middle whitespace-nowrap first:pl-5 last:pr-5', className)}
    {...props}
  />
)
