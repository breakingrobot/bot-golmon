// Dialogue « Nouvelle présence » : publiée sur Discord dans le salon des présences, avec le ping habituel.

import { Plus } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useCreatePresence } from '@/api/queries-groupe'
import type { NewPresence } from '@/api/types'
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
import { Textarea } from '@/atoms/Textarea'
import { todayParis } from '@/lib/format'
import { normalizeTime } from '@/lib/presence'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'

const TITLE_MAX = 100
const DESCRIPTION_MAX = 1000

type FormErrors = Partial<Record<'date' | 'time' | 'title' | 'description', string>>

export type NewPresenceDialogProps = { guildId: string; onCreated?: (id: string) => void }

export const NewPresenceDialog = ({ guildId, onCreated }: NewPresenceDialogProps) => {
  const [open, setOpen] = useState(false)
  const [date, setDate] = useState('')
  const [time, setTime] = useState('21:00')
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [errors, setErrors] = useState<FormErrors>({})
  const mutation = useCreatePresence(guildId)

  const reset = () => {
    setDate('')
    setTime('21:00')
    setTitle('')
    setDescription('')
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: FormErrors = {}
    const normalized = normalizeTime(time)
    if (!normalized) next.time = 'Heure au format HH:MM.'
    if (date && date < todayParis()) next.date = 'Cette date est déjà passée.'
    if (title.trim().length > TITLE_MAX) next.title = `${TITLE_MAX} caractères au plus.`
    if (description.trim().length > DESCRIPTION_MAX) next.description = `${DESCRIPTION_MAX} caractères au plus.`
    setErrors(next)
    if (Object.keys(next).length > 0 || !normalized) return
    const body: NewPresence = {
      time: normalized,
      ...(date ? { date } : {}),
      ...(title.trim() ? { title: title.trim() } : {}),
      ...(description.trim() ? { description: description.trim() } : {}),
    }
    mutation.mutate(body, {
      onSuccess: (presence) => {
        setOpen(false)
        reset()
        onCreated?.(presence.id)
      },
    })
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) reset()
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus aria-hidden /> Nouvelle présence
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nouvelle présence</DialogTitle>
          <DialogDescription>
            Publiée dans le salon des présences : le rôle est pingé, sauf les absents et la blocklist. Heure de Paris.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Jour" error={errors.date} hint="Vide : aujourd'hui, ou demain si l'heure est passée.">
              {(props) => (
                <Input
                  {...props}
                  type="date"
                  value={date}
                  min={todayParis()}
                  onChange={(event) => setDate(event.target.value)}
                />
              )}
            </Field>
            <Field label="Heure" error={errors.time}>
              {(props) => (
                <Input {...props} type="time" value={time} onChange={(event) => setTime(event.target.value)} required />
              )}
            </Field>
          </div>
          <Field label="Titre" error={errors.title} hint={<CharCount value={title} max={TITLE_MAX} />}>
            {(props) => (
              <Input
                {...props}
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="Présence"
                autoFocus
              />
            )}
          </Field>
          <Field
            label="Description"
            error={errors.description}
            hint={<CharCount value={description} max={DESCRIPTION_MAX} />}
          >
            {(props) => (
              <Textarea
                {...props}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                rows={3}
              />
            )}
          </Field>
          {mutation.isError && (
            <p role="alert" className="rounded-md border border-danger/40 bg-danger-soft px-3 py-2 text-sm text-danger">
              {errorMessage(mutation.error)}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? 'Publication…' : 'Publier'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
