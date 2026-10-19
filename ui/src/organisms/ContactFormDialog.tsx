import { useState } from 'react'
import type { FormEvent, ReactNode } from 'react'

import { useCreateContact, useUpdateContact } from '@/api/queries'
import type { Contact, ContactInput, ContactKind, Relation } from '@/api/types'
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { Textarea } from '@/atoms/Textarea'
import { KIND_LABELS, LIMITS, RELATION_LABELS } from '@/lib/contact'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'

type FormErrors = Partial<Record<'name' | 'phone' | 'description', string>>

export type ContactFormDialogProps = {
  guildId: string
  /** Absent : création. */
  contact?: Contact
  trigger: ReactNode
  onSaved?: (contact: Contact) => void
}

export const ContactFormDialog = ({ guildId, contact, trigger, onSaved }: ContactFormDialogProps) => {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [kind, setKind] = useState<ContactKind>('GROUPE')
  const [relation, setRelation] = useState<Relation>('NEUTRE')
  const [phone, setPhone] = useState('')
  const [description, setDescription] = useState('')
  const [errors, setErrors] = useState<FormErrors>({})
  const create = useCreateContact(guildId)
  const update = useUpdateContact(guildId)
  const pending = create.isPending || update.isPending

  const load = () => {
    setName(contact?.name ?? '')
    setKind(contact?.kind ?? 'GROUPE')
    setRelation(contact?.relation ?? 'NEUTRE')
    setPhone(contact?.phone ?? '')
    setDescription(contact?.description ?? '')
    setErrors({})
  }

  const done = (saved: Contact) => {
    setOpen(false)
    onSaved?.(saved)
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: FormErrors = {}
    if (!name.trim()) next.name = 'Le nom est obligatoire.'
    if (name.trim().length > LIMITS.name) next.name = `${LIMITS.name} caractères au plus.`
    if (phone.trim().length > LIMITS.phone) next.phone = `${LIMITS.phone} caractères au plus.`
    if (description.trim().length > LIMITS.description) next.description = `${LIMITS.description} caractères au plus.`
    setErrors(next)
    if (Object.keys(next).length > 0) return
    if (contact) {
      // Seuls les champs modifiés sont envoyés ; téléphone ou description vides = effacer (null).
      const body: Partial<ContactInput> = {}
      if (name.trim() !== contact.name) body.name = name.trim()
      if (kind !== contact.kind) body.kind = kind
      if (relation !== contact.relation) body.relation = relation
      if ((phone.trim() || null) !== contact.phone) body.phone = phone.trim() || null
      if ((description.trim() || null) !== contact.description) body.description = description.trim() || null
      if (Object.keys(body).length === 0) {
        setOpen(false)
        return
      }
      update.mutate({ id: contact.id, body }, { onSuccess: done })
    } else {
      create.mutate(
        {
          name: name.trim(),
          kind,
          relation,
          ...(phone.trim() ? { phone: phone.trim() } : {}),
          ...(description.trim() ? { description: description.trim() } : {}),
        },
        { onSuccess: done },
      )
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) load()
      }}
    >
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{contact ? `Modifier « ${contact.name} »` : 'Nouveau contact'}</DialogTitle>
          <DialogDescription>
            Le répertoire publié sur Discord est mis à jour, et l'action inscrite au journal.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <Field label="Nom" error={errors.name} hint={<CharCount value={name} max={LIMITS.name} />}>
            {(props) => (
              <Input
                {...props}
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={LIMITS.name + 20}
                autoFocus
              />
            )}
          </Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Type">
              {(props) => (
                <Select value={kind} onValueChange={(value) => setKind(value as ContactKind)}>
                  <SelectTrigger {...props}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {(Object.keys(KIND_LABELS) as ContactKind[]).map((value) => (
                      <SelectItem key={value} value={value}>
                        {KIND_LABELS[value]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </Field>
            <Field label="Relation">
              {(props) => (
                <Select value={relation} onValueChange={(value) => setRelation(value as Relation)}>
                  <SelectTrigger {...props}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {(Object.keys(RELATION_LABELS) as Relation[]).map((value) => (
                      <SelectItem key={value} value={value}>
                        {RELATION_LABELS[value]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </Field>
          </div>
          <Field label="Téléphone" error={errors.phone} hint="Facultatif.">
            {(props) => (
              <Input
                {...props}
                value={phone}
                onChange={(event) => setPhone(event.target.value)}
                inputMode="tel"
                placeholder="555-0123"
              />
            )}
          </Field>
          <Field
            label="Description"
            error={errors.description}
            hint={<CharCount value={description} max={LIMITS.description} />}
          >
            {(props) => (
              <Textarea
                {...props}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                rows={4}
              />
            )}
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" disabled={pending}>
              {pending ? 'Enregistrement…' : contact ? 'Enregistrer' : 'Ajouter'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
