import { Trash2 } from 'lucide-react'

import { useDeletePhone } from '@/api/queries-groupe'
import type { PhoneEntry } from '@/api/types'
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

export type DeletePhoneProps = { guildId: string; entry: PhoneEntry; own: boolean }

export const DeletePhone = ({ guildId, entry, own }: DeletePhoneProps) => {
  const mutation = useDeletePhone(guildId)
  const whose = own ? 'votre numéro' : `le numéro de ${entry.member.name}`
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="ghost" size="icon-sm" aria-label={`Supprimer ${whose}`} title="Supprimer">
          <Trash2 aria-hidden />
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Supprimer {whose} ?</AlertDialogTitle>
          <AlertDialogDescription>
            {entry.number} disparaît de l'annuaire, sur le site et sur Discord.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction disabled={mutation.isPending} onClick={() => mutation.mutate(entry.member.id)}>
            Supprimer
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
