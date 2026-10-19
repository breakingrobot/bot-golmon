import type { ObjectiveProgress } from '@/api/types'
import { formatMoney } from '@/lib/format'
import { ShareBar } from '@/molecules/ShareBar'

export type ObjectiveBarProps = { progress: ObjectiveProgress; className?: string }

export const ObjectiveBar = ({ progress, className }: ObjectiveBarProps) => {
  const tone = progress.exempt
    ? 'brand'
    : progress.reached
      ? 'staff'
      : progress.amount / progress.target >= 0.5
        ? 'internal'
        : 'danger'
  return (
    <div className={className}>
      <ShareBar
        value={Math.min(progress.amount, progress.target)}
        max={progress.target}
        label={`Objectif de ${progress.member.name}`}
        tone={tone}
        className="w-full"
      />
      <p className="mt-1 text-xs text-ink-muted tabular-nums">
        {formatMoney(progress.amount)} / {formatMoney(progress.target)}
      </p>
    </div>
  )
}
