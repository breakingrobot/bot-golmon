import { cn } from '@/lib/utils'

export type CharCountProps = { value: string; max: number }

/** Compteur de caractères « 12 / 80 ». */
export const CharCount = ({ value, max }: CharCountProps) => (
  <span className={cn('text-xs tabular-nums', value.length > max ? 'text-danger' : 'text-ink-subtle')}>
    {value.length} / {max}
  </span>
)
