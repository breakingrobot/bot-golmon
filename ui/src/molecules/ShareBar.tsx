import { cn } from '@/lib/utils'

/** Barre de part (pourcentage), couleur selon seuils optionnels. */
export function ShareBar({
  value,
  max = 100,
  label,
  tone = 'brand',
  className,
}: {
  value: number
  max?: number
  label: string
  tone?: 'brand' | 'staff' | 'internal' | 'danger'
  className?: string
}) {
  const percent = max > 0 ? Math.max(0, Math.min(100, (value / max) * 100)) : 0
  const fill = { brand: 'bg-brand', staff: 'bg-staff', internal: 'bg-internal', danger: 'bg-danger' }[tone]
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value}
      className={cn('h-2 w-24 overflow-hidden rounded-full bg-muted', className)}
    >
      <div className={cn('h-full rounded-full', fill)} style={{ width: `${percent}%` }} />
    </div>
  )
}
