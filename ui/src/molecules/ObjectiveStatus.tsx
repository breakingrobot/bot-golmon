import { CircleCheck, CircleMinus } from 'lucide-react'

import type { ObjectiveProgress } from '@/api/types'
import { Badge } from '@/atoms/Badge'

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
