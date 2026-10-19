import type { ReactNode } from 'react'

import { type Tone, toneClass } from '@/lib/tone'
import { cn } from '@/lib/utils'

export function StatCard({
  label,
  value,
  hint,
  tone = 'default',
  className,
}: {
  label: ReactNode
  value: ReactNode
  hint?: ReactNode
  tone?: Tone
  className?: string
}) {
  return (
    <div className={cn('rounded-xl border bg-card px-4 py-3 shadow-xs', className)}>
      <dt className="text-xs font-medium tracking-wide text-ink-subtle uppercase">{label}</dt>
      <dd className={cn('mt-1 text-xl font-semibold tabular-nums', toneClass[tone])}>{value}</dd>
      {hint && <dd className="mt-0.5 text-xs text-ink-muted">{hint}</dd>}
    </div>
  )
}
