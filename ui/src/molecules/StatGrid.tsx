import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

export type StatGridProps = { children: ReactNode; className?: string }

export const StatGrid = ({ children, className }: StatGridProps) => (
  <dl className={cn('grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4', className)}>{children}</dl>
)
