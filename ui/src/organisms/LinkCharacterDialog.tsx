import { Link2 } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useLinkPlayer } from '@/api/queries'
import type { MemberRef } from '@/api/types'
import { Button } from '@/atoms/Button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/atoms/Dialog'
import { Field } from '@/molecules/Field'
import { MemberPicker } from '@/molecules/MemberPicker'

export type LinkCharacterDialogProps = {
  guildId: string
  gameId: string
  name: string
  current: MemberRef | null
}

export const LinkCharacterDialog = ({ guildId, gameId, name, current }: LinkCharacterDialogProps) => {
  const [open, setOpen] = useState(false)
  const [member, setMember] = useState<MemberRef | null>(current)
  const [error, setError] = useState<string | null>(null)
  const mutation = useLinkPlayer(guildId)
  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (!member) {
      setError('Choisissez le membre Discord.')
      return
    }
    // Refus du serveur (personne sans le rôle membre…) affiché sous le champ, en plus de la notification.
    mutation.mutate(
      { gameId, memberId: member.id },
      { onSuccess: () => setOpen(false), onError: (failure) => setError(errorMessage(failure)) },
    )
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        setMember(current)
        setError(null)
        mutation.reset()
      }}
    >
      <DialogTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={`${current ? 'Changer le membre relié à' : 'Relier'} ${name}`}
        >
          <Link2 aria-hidden />
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Relier un personnage</DialogTitle>
          <DialogDescription>
            {name} (identifiant {gameId}) : ses dépôts et retraits apparaîtront sur la fiche du membre.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit}>
          <Field label="Membre Discord" error={error}>
            {(props) => (
              <MemberPicker
                guildId={guildId}
                value={member}
                onChange={setMember}
                id={props.id}
                invalid={Boolean(error)}
              />
            )}
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              Relier
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
