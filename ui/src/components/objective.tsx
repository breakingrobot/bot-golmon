import { CircleCheck, CircleMinus } from 'lucide-react'

import type { Objective, ObjectiveProgress } from '@/api/types'
import { ShareBar } from '@/components/page'
import { Badge } from '@/components/ui/badge'
import { formatMoney } from '@/lib/format'

export const COUNTS_LABEL: Record<Objective['counts'], string> = {
  total: 'propre + sale',
  propre: 'argent propre seulement',
  sale: 'argent sale seulement',
}

/** Avancement d'un membre vers l'objectif de la semaine. */
export function ObjectiveStatus({ progress }: { progress: ObjectiveProgress }) {
  if (progress.exempt) {
    return (
      <Badge variant="neutral">
        <CircleMinus aria-hidden /> Non concerné
      </Badge>
    )
  }
  if (progress.reached) {
    return (
      <Badge variant="success">
        <CircleCheck aria-hidden /> Atteint
      </Badge>
    )
  }
  return <Badge variant="warning">En cours</Badge>
}

export function ObjectiveBar({ progress, className }: { progress: ObjectiveProgress; className?: string }) {
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
