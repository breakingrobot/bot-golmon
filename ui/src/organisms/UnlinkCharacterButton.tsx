import { Unlink } from 'lucide-react'
import { useState } from 'react'

import { useUnlinkPlayer } from '@/api/queries'
import type { MemberRef } from '@/api/types'
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

export function UnlinkCharacterButton({
  guildId,
  gameId,
  name,
  member,
}: {
  guildId: string
  gameId: string
  name: string
  member: MemberRef
}) {
  const [open, setOpen] = useState(false)
  const mutation = useUnlinkPlayer(guildId)
  return (
    <AlertDialog open={open} onOpenChange={setOpen}>
      <AlertDialogTrigger asChild>
        <Button variant="ghost" size="icon-sm" aria-label={`Délier ${name} de ${member.name}`}>
          <Unlink aria-hidden />
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Délier ce personnage ?</AlertDialogTitle>
          <AlertDialogDescription>
            {name} ne sera plus relié à {member.name}. L'historique des mouvements est conservé.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction
            disabled={mutation.isPending}
            onClick={(event) => {
              event.preventDefault()
              mutation.mutate(gameId, { onSuccess: () => setOpen(false) })
            }}
          >
            Délier
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
