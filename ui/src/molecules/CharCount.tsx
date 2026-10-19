import { cn } from '@/lib/utils'

/** Compteur de caractères « 12 / 80 ». */
export function CharCount({ value, max }: { value: string; max: number }) {
  return (
    <span className={cn('text-xs tabular-nums', value.length > max ? 'text-danger' : 'text-ink-subtle')}>
      {value.length} / {max}
    </span>
  )
}
