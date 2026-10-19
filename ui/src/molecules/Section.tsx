import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

export function Section({
  title,
  description,
  actions,
  children,
  className,
  id,
}: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  children: ReactNode
  className?: string
  id?: string
}) {
  const headingId = id ? `${id}-titre` : undefined
  return (
    <section className={cn('mt-10 first:mt-0', className)} aria-labelledby={headingId} id={id}>
      <div className="mb-3 flex flex-wrap items-end justify-between gap-x-4 gap-y-2">
        <div>
          <h2 id={headingId} className="text-lg font-semibold text-ink">
            {title}
          </h2>
          {description && <p className="mt-0.5 text-sm text-ink-muted">{description}</p>}
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
      {children}
    </section>
  )
}
