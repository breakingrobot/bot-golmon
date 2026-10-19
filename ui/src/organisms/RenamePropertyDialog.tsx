import { Pencil } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { useRenameProperty } from '@/api/queries'
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
import { Input } from '@/atoms/Input'
import { propertyLabel } from '@/lib/coffre'
import { Field } from '@/molecules/Field'

export type RenamePropertyDialogProps = {
  guildId: string
  property: { id: string; name: string | null }
}

export const RenamePropertyDialog = ({ guildId, property }: RenamePropertyDialogProps) => {
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
