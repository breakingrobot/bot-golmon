import { Trash2 } from 'lucide-react'
import { useState } from 'react'

import { useDeleteContact } from '@/api/queries'
import type { Contact } from '@/api/types'
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
import { Input } from '@/atoms/Input'
import { useGuild } from '@/lib/guild-context'

export function DeleteContact({ contact, onDeleted }: { contact: Contact; onDeleted: () => void }) {
  const { guild } = useGuild()
  const mutation = useDeleteContact(guild.id)
  const [confirm, setConfirm] = useState('')
  return (
    <AlertDialog onOpenChange={() => setConfirm('')}>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size="sm" className="text-danger hover:text-danger">
          <Trash2 aria-hidden /> Supprimer
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Supprimer « {contact.name} » ?</AlertDialogTitle>
          <AlertDialogDescription>
            Le contact et ses notes disparaissent du répertoire, sur le site et sur Discord. Tapez le nom pour
            confirmer.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <Input value={confirm} onChange={(event) => setConfirm(event.target.value)} aria-label="Nom du contact" />
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction
            disabled={confirm.trim() !== contact.name || mutation.isPending}
            onClick={(event) => {
              event.preventDefault()
              mutation.mutate(contact.id, { onSuccess: onDeleted })
            }}
          >
            Supprimer
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
