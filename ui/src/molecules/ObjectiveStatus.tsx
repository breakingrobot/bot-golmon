import { CircleCheck, CircleMinus } from 'lucide-react'

import type { ObjectiveProgress } from '@/api/types'
import { Badge } from '@/atoms/Badge'

export type ObjectiveStatusProps = { progress: ObjectiveProgress }

/** Avancement d'un membre vers l'objectif de la semaine. */
export const ObjectiveStatus = ({ progress }: ObjectiveStatusProps) => {
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
