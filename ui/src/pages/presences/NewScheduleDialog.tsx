// Dialogue « Planifier une présence » : publiée chaque semaine, quelques heures avant l'événement.

import { CalendarPlus } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useCreatePresenceSchedule } from '@/api/queries-groupe'
import type { NewPresenceSchedule } from '@/api/types'
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
import { normalizeTime, WEEKDAYS } from '@/lib/presence'
import { Field } from '@/molecules/Field'

type FormErrors = Partial<Record<'time' | 'title' | 'lead', string>>

export type NewScheduleDialogProps = { guildId: string }

export const NewScheduleDialog = ({ guildId }: NewScheduleDialogProps) => {
  const [open, setOpen] = useState(false)
  const [weekday, setWeekday] = useState('3')
  const [time, setTime] = useState('21:00')
  const [title, setTitle] = useState('')
  const [lead, setLead] = useState('24')
  const [errors, setErrors] = useState<FormErrors>({})
  const mutation = useCreatePresenceSchedule(guildId)

  const reset = () => {
    setWeekday('3')
    setTime('21:00')
    setTitle('')
    setLead('24')
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: FormErrors = {}
    const normalized = normalizeTime(time)
    const leadHours = Number(lead)
    if (!normalized) next.time = 'Heure au format HH:MM.'
    if (!Number.isInteger(leadHours) || leadHours < 1 || leadHours > 144) next.lead = 'Entre 1 et 144 heures.'
    if (title.trim().length > 100) next.title = '100 caractères au plus.'
    setErrors(next)
    if (Object.keys(next).length > 0 || !normalized) return
    const body: NewPresenceSchedule = {
      weekday: Number(weekday),
      time: normalized,
      leadHours,
      ...(title.trim() ? { title: title.trim() } : {}),
    }
    mutation.mutate(body, {
      onSuccess: () => {
        setOpen(false)
        reset()
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
        <Button size="sm" variant="outline">
          <CalendarPlus aria-hidden /> Planifier
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Planifier une présence</DialogTitle>
          <DialogDescription>
            Publiée automatiquement chaque semaine dans le salon des présences, avec le ping habituel. Heure de Paris.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Jour">
              {(props) => (
                <Select value={weekday} onValueChange={setWeekday}>
                  <SelectTrigger {...props}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {WEEKDAYS.map((day, index) => (
                      <SelectItem key={day} value={String(index + 1)} className="capitalize">
                        {day}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </Field>
            <Field label="Heure" error={errors.time}>
              {(props) => (
                <Input {...props} type="time" value={time} onChange={(event) => setTime(event.target.value)} />
              )}
            </Field>
          </div>
          <Field label="Titre" error={errors.title} hint="Vide : « Présence ».">
            {(props) => (
              <Input
                {...props}
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="Présence"
              />
            )}
          </Field>
          <Field label="Publication (heures avant)" error={errors.lead} hint="24 par défaut, 144 au plus.">
            {(props) => (
              <Input {...props} inputMode="numeric" value={lead} onChange={(event) => setLead(event.target.value)} />
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
              {mutation.isPending ? 'Enregistrement…' : 'Planifier'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
