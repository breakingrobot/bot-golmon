// Gestion des coffres (`coffre.gerer`) : nommer une propriété, relier ou délier un personnage.

import { Link2, Pencil, Unlink } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useLinkPlayer, useRenameProperty, useUnlinkPlayer } from '@/api/queries'
import type { MemberRef } from '@/api/types'
import { Field } from '@/components/form'
import { MemberPicker } from '@/components/member-picker'
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
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

export function propertyLabel(property: { id: string; name: string | null }): string {
  return property.name ?? `Propriété n° ${property.id}`
}

export function RenamePropertyDialog({
  guildId,
  property,
}: {
  guildId: string
  property: { id: string; name: string | null }
}) {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState(property.name ?? '')
  const mutation = useRenameProperty(guildId)
  const submit = (event: FormEvent) => {
    event.preventDefault()
    mutation.mutate({ propertyId: property.id, name: name.trim() }, { onSuccess: () => setOpen(false) })
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) setName(property.name ?? '')
      }}
    >
      <DialogTrigger asChild>
        <Button variant="ghost" size="icon-sm" aria-label={`Renommer ${propertyLabel(property)}`}>
          <Pencil aria-hidden />
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nommer la propriété</DialogTitle>
          <DialogDescription>Propriété n° {property.id} du jeu. Laisser vide pour retirer le nom.</DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit}>
          <Field label="Nom" hint="Affiché à la place du numéro, sur Discord et sur le site.">
            {(props) => (
              <Input
                {...props}
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="Planque du port"
                autoFocus
              />
            )}
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              Enregistrer
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export function LinkCharacterDialog({
  guildId,
  gameId,
  name,
  current,
}: {
  guildId: string
  gameId: string
  name: string
  current: MemberRef | null
}) {
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
