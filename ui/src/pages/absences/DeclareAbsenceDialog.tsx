// Dialogue « Déclarer une absence » : pour soi, ou pour un membre avec `absence.autrui`.

import { CalendarPlus } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useDeclareAbsence } from '@/api/queries-groupe'
import type { MemberRef, NewAbsence } from '@/api/types'
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
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { todayParis } from '@/lib/format'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'
import { MemberPicker } from '@/molecules/MemberPicker'

const MOTIF_MAX = 200

type FormErrors = Partial<Record<'member' | 'motif' | 'from' | 'to', string>>

/** Erreurs de saisie, mêmes règles que `/absence declare` (dates en jours de Paris). */
export const validateAbsence = (
  form: { motif: string; from: string; to: string; forOther: boolean; member: MemberRef | null },
  today: string,
): FormErrors => {
  const errors: FormErrors = {}
  if (form.forOther && !form.member) errors.member = 'Choisissez le membre.'
  if (!form.motif.trim()) errors.motif = 'Indiquez un motif.'
  else if (form.motif.trim().length > MOTIF_MAX) errors.motif = `${MOTIF_MAX} caractères au plus.`
  if (!form.to) errors.to = 'Indiquez le dernier jour.'
  else if (form.to < today) errors.to = 'Cette absence est déjà terminée.'
  else if (form.from && form.to < form.from) errors.to = 'Le dernier jour doit suivre le premier.'
  return errors
}

export type DeclareAbsenceDialogProps = {
  guildId: string
  /** `absence.autrui` : déclarer pour un autre membre. */
  canManage: boolean
}

export const DeclareAbsenceDialog = ({ guildId, canManage }: DeclareAbsenceDialogProps) => {
  const [open, setOpen] = useState(false)
  const [forOther, setForOther] = useState(false)
  const [member, setMember] = useState<MemberRef | null>(null)
  const [motif, setMotif] = useState('')
  const [from, setFrom] = useState(todayParis())
  const [to, setTo] = useState('')
  const [errors, setErrors] = useState<FormErrors>({})
  const mutation = useDeclareAbsence(guildId)

  const reset = () => {
    setForOther(false)
    setMember(null)
    setMotif('')
    setFrom(todayParis())
    setTo('')
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next = validateAbsence({ motif, from, to, forOther, member }, todayParis())
    setErrors(next)
    if (Object.keys(next).length > 0) return
    const body: NewAbsence = {
      motif: motif.trim(),
      to,
      ...(from && from !== todayParis() ? { from } : {}),
      ...(forOther && member ? { memberId: member.id } : {}),
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
        <Button size="sm">
          <CalendarPlus aria-hidden /> Déclarer une absence
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Déclarer une absence</DialogTitle>
          <DialogDescription>
            Journées entières, heure de Paris. Le rôle d'absence est posé dès le premier jour, les présences ouvertes de
            la période passent en absent et l'absence est annoncée sur Discord.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          {canManage && (
            <div className="grid gap-2">
              <span className="text-sm font-medium text-ink-muted" id="absence-pour">
                Pour
              </span>
              <ToggleGroup
                type="single"
                value={forOther ? 'autre' : 'moi'}
                onValueChange={(value) => value && setForOther(value === 'autre')}
                aria-labelledby="absence-pour"
              >
                <ToggleGroupItem value="moi">Moi-même</ToggleGroupItem>
                <ToggleGroupItem value="autre">Un autre membre</ToggleGroupItem>
              </ToggleGroup>
              {forOther && (
                <Field label="Membre" error={errors.member}>
                  {(props) => (
                    <MemberPicker
                      guildId={guildId}
                      value={member}
                      onChange={setMember}
                      id={props.id}
                      invalid={Boolean(errors.member)}
                    />
                  )}
                </Field>
              )}
            </div>
          )}
          <Field label="Motif" error={errors.motif} hint={<CharCount value={motif} max={MOTIF_MAX} />}>
            {(props) => (
              <Input
                {...props}
                value={motif}
                onChange={(event) => setMotif(event.target.value)}
                placeholder="Vacances, travail…"
                autoFocus
              />
            )}
          </Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Premier jour" error={errors.from}>
              {(props) => (
                <Input {...props} type="date" value={from} onChange={(event) => setFrom(event.target.value)} />
              )}
            </Field>
            <Field label="Dernier jour" error={errors.to}>
              {(props) => (
                <Input
                  {...props}
                  type="date"
                  value={to}
                  min={from || todayParis()}
                  onChange={(event) => setTo(event.target.value)}
                />
              )}
            </Field>
          </div>
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
              {mutation.isPending ? 'Enregistrement…' : 'Déclarer'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
