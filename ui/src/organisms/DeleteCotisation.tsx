import { Trash2 } from 'lucide-react'
import { useState } from 'react'

import { useDeleteCotisation } from '@/api/queries'
import type { CotisationEntry } from '@/api/types'
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
import { formatDay, formatMoney } from '@/lib/format'
import { useGuild } from '@/lib/guild-context'

export function DeleteCotisation({ entry }: { entry: CotisationEntry }) {
  const { guild } = useGuild()
  const mutation = useDeleteCotisation(guild.id)
  const [open, setOpen] = useState(false)
  return (
    <AlertDialog open={open} onOpenChange={setOpen}>
      <AlertDialogTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={`Supprimer la cotisation du ${formatDay(entry.date)} de ${entry.member.name}`}
        >
          <Trash2 aria-hidden />
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Supprimer cette cotisation ?</AlertDialogTitle>
          <AlertDialogDescription>
            {entry.member.name}, {formatDay(entry.date)} : {formatMoney(entry.propre + entry.sale)}. La suppression est
            inscrite au journal du staff.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction
            disabled={mutation.isPending}
            onClick={(event) => {
              event.preventDefault()
              mutation.mutate(entry.id, { onSuccess: () => setOpen(false) })
            }}
          >
            Supprimer
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
