// Dialogue « Mon numéro » : le sien, ou celui d'un membre avec `numero.ajouter_autrui`.

import { Phone } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useSetPhone } from '@/api/queries-groupe'
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
import { Input } from '@/atoms/Input'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { Field } from '@/molecules/Field'
import { MemberPicker } from '@/molecules/MemberPicker'

/** Format du jeu, comme `/numero` : 555 suivi de 4 à 6 chiffres (`555-1234` accepté). */
export const validPhone = (text: string): boolean => /^555\d{4,6}$/u.test(text.replaceAll(/\D/gu, ''))

export type PhoneDialogProps = {
  guildId: string
  userId: string
  /** Numéro actuel de la personne connectée. */
  current: string | null
  /** `numero.ajouter_autrui`. */
  canManage: boolean
}

export const PhoneDialog = ({ guildId, userId, current, canManage }: PhoneDialogProps) => {
  const [open, setOpen] = useState(false)
  const [forOther, setForOther] = useState(false)
  const [member, setMember] = useState<MemberRef | null>(null)
  const [number, setNumber] = useState('')
  const [errors, setErrors] = useState<{ member?: string; number?: string }>({})
  const mutation = useSetPhone(guildId)

  const load = () => {
    setForOther(false)
    setMember(null)
    setNumber(current ?? '')
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: { member?: string; number?: string } = {}
    if (forOther && !member) next.member = 'Choisissez le membre.'
    if (!validPhone(number)) next.number = '555 suivi de 4 à 6 chiffres, par exemple 555-1234.'
    setErrors(next)
    if (Object.keys(next).length > 0) return
    const target = forOther && member ? member.id : userId
    mutation.mutate({ userId: target, number: number.trim() }, { onSuccess: () => setOpen(false) })
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) load()
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">
          <Phone aria-hidden /> {current ? 'Modifier mon numéro' : 'Ajouter mon numéro'}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Numéro de téléphone</DialogTitle>
          <DialogDescription>
            Un numéro par membre, unique sur le serveur. L'annuaire publié sur Discord est mis à jour.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          {canManage && (
            <div className="grid gap-2">
              <span className="text-sm font-medium text-ink-muted" id="numero-pour">
                Pour
              </span>
              <ToggleGroup
                type="single"
                value={forOther ? 'autre' : 'moi'}
                onValueChange={(value) => value && setForOther(value === 'autre')}
                aria-labelledby="numero-pour"
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
          <Field label="Numéro" error={errors.number}>
            {(props) => (
              <Input
                {...props}
                inputMode="tel"
                value={number}
                onChange={(event) => setNumber(event.target.value)}
                placeholder="555-1234"
                autoFocus
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
              {mutation.isPending ? 'Enregistrement…' : 'Enregistrer'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
