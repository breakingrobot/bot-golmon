import { CalendarX } from 'lucide-react'

import { useEndAbsence } from '@/api/queries-groupe'
import type { AbsenceEntry } from '@/api/types'
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

export type EndAbsenceButtonProps = {
  guildId: string
  absence: AbsenceEntry
  /** L'absence n'a pas commencé : elle est annulée plutôt que terminée. */
  upcoming: boolean
  /** Absence de la personne connectée. */
  own: boolean
}

/** Terminer une absence en cours ou annuler une absence à venir, comme `/absence end`. */
export const EndAbsenceButton = ({ guildId, absence, upcoming, own }: EndAbsenceButtonProps) => {
  const mutation = useEndAbsence(guildId)
  const verb = upcoming ? 'Annuler' : 'Terminer'
  const whose = own ? 'votre absence' : `l'absence de ${absence.member.name}`
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size="sm" aria-label={`${verb} ${whose}`}>
          <CalendarX aria-hidden /> {verb}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {verb} {whose} ?
          </AlertDialogTitle>
          <AlertDialogDescription>
            {upcoming
              ? "L'absence n'a pas commencé : elle est supprimée, avec son annonce sur Discord."
              : "Le rôle d'absence est retiré (sauf autre absence en cours) et l'annonce supprimée sur Discord."}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Retour</AlertDialogCancel>
          <AlertDialogAction disabled={mutation.isPending} onClick={() => mutation.mutate(absence.id)}>
            {verb}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
