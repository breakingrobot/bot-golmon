import { Trash2 } from 'lucide-react'

import { useDeletePresenceSchedule } from '@/api/queries-groupe'
import type { PresenceSchedule } from '@/api/types'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/atoms/AlertDialog'
import { Button } from '@/atoms/Button'

export type DeleteScheduleProps = { guildId: string; schedule: PresenceSchedule }

export const DeleteSchedule = ({ guildId, schedule }: DeleteScheduleProps) => {
  const mutation = useDeletePresenceSchedule(guildId)
  const label = `${schedule.weekdayLabel} à ${schedule.time}`
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="ghost" size="icon-sm" aria-label={`Supprimer le planning du ${label}`} title="Supprimer">
          <Trash2 aria-hidden />
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            Supprimer « {schedule.title} » du {label} ?
          </AlertDialogTitle>
          <AlertDialogDescription>
            Plus aucune présence ne sera publiée pour ce planning. Les présences déjà publiées restent en place.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction disabled={mutation.isPending} onClick={() => mutation.mutate(schedule.id)}>
            Supprimer
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
