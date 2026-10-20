import { Trash2 } from 'lucide-react'

import { useDeleteVehicle } from '@/api/queries-groupe'
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

export type DeleteVehicleProps = { guildId: string; plate: string }

export const DeleteVehicle = ({ guildId, plate }: DeleteVehicleProps) => {
  const mutation = useDeleteVehicle(guildId)
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="ghost" size="icon-sm" aria-label={`Retirer ${plate} du registre`} title="Retirer">
          <Trash2 aria-hidden />
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Retirer {plate} du registre ?</AlertDialogTitle>
          <AlertDialogDescription>
            Le véhicule disparaît du tableau de bord publié sur Discord, comme avec /vehicle remove.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction disabled={mutation.isPending} onClick={() => mutation.mutate(plate)}>
            Retirer
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
