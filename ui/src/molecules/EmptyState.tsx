import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

export type EmptyStateProps = {
  title: ReactNode
  children?: ReactNode
  icon?: ReactNode
  className?: string
}

export const EmptyState = ({ title, children, icon, className }: EmptyStateProps) => (
  <div
    className={cn(
      'flex flex-col items-center rounded-xl border border-dashed border-line-strong px-6 py-10 text-center',
      className,
    )}
  >
    {icon && <div className="mb-3 text-ink-subtle [&_svg]:size-6">{icon}</div>}
    <p className="font-medium text-ink">{title}</p>
    {children && <div className="mt-1 max-w-md text-sm text-ink-muted">{children}</div>}
  </div>
)
