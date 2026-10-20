import type { PresenceSummary } from '@/api/types'
import { formatNumber } from '@/lib/format'
import { cn } from '@/lib/utils'

export type AnswerCountsProps = {
  presence: Pick<PresenceSummary, 'present' | 'absent' | 'retard' | 'noAnswer'>
  className?: string
}

/** Décompte d'une présence, comme le message Discord : ✅ ❌ ⏳ et ❔ sans réponse. */
export const AnswerCounts = ({ presence, className }: AnswerCountsProps) => {
  const counts = [
    { emoji: '✅', value: presence.present, label: 'présent(s)', tone: 'text-staff' },
    { emoji: '❌', value: presence.absent, label: 'absent(s)', tone: 'text-danger' },
    { emoji: '⏳', value: presence.retard, label: 'retard(s)', tone: 'text-internal' },
    { emoji: '❔', value: presence.noAnswer, label: 'sans réponse', tone: 'text-ink-muted' },
  ]
  return (
    <span className={cn('flex items-center gap-3 text-sm tabular-nums', className)}>
      {counts.map((count) => (
        <span key={count.label} className={count.tone}>
          <span aria-hidden>{count.emoji}</span> {formatNumber(count.value)}
          <span className="sr-only"> {count.label}</span>
        </span>
      ))}
    </span>
  )
}
