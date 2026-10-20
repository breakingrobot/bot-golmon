// Dialogue d'avis sur une recrue : vote et motif obligatoire, comme le formulaire Discord.

import { Vote } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useVoteRecrue } from '@/api/queries-groupe'
import type { Recrue, RecrueChoice } from '@/api/types'
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
import { Textarea } from '@/atoms/Textarea'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { CHOICES, MOTIF_MAX } from '@/lib/recrue'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'

export type VoteDialogProps = { guildId: string; recrue: Recrue }

export const VoteDialog = ({ guildId, recrue }: VoteDialogProps) => {
  const [open, setOpen] = useState(false)
  const [choice, setChoice] = useState<RecrueChoice | ''>('')
  const [motif, setMotif] = useState('')
  const [errors, setErrors] = useState<{ choice?: string; motif?: string }>({})
  const mutation = useVoteRecrue(guildId)

  const load = () => {
    setChoice(recrue.myVote?.choice ?? '')
    setMotif(recrue.myVote?.motif ?? '')
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: { choice?: string; motif?: string } = {}
    if (!choice) next.choice = 'Choisissez votre avis.'
    if (!motif.trim()) next.motif = 'Le motif est obligatoire.'
    else if (motif.trim().length > MOTIF_MAX) next.motif = `${MOTIF_MAX} caractères au plus.`
    setErrors(next)
    if (Object.keys(next).length > 0 || !choice) return
    mutation.mutate(
      { entryId: recrue.entryId, body: { choice, motif: motif.trim() } },
      { onSuccess: () => setOpen(false) },
    )
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
        <Button size="sm" variant={recrue.myVote ? 'outline' : 'default'}>
          <Vote aria-hidden /> {recrue.myVote ? 'Modifier mon avis' : 'Donner mon avis'}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Avis sur {recrue.member.name}</DialogTitle>
          <DialogDescription>
            Un avis par gradé, modifiable jusqu'à la fin de la semaine. Le motif est visible par les gradés ; le message
            d'avis sur Discord est mis à jour.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <div className="grid gap-2">
            <span className="text-sm font-medium text-ink-muted" id={`avis-${recrue.entryId}`}>
              Avis
            </span>
            <ToggleGroup
              type="single"
              value={choice}
              onValueChange={(value) => setChoice(value as RecrueChoice | '')}
              aria-labelledby={`avis-${recrue.entryId}`}
            >
              {CHOICES.map((item) => (
                <ToggleGroupItem key={item.value} value={item.value}>
                  <span aria-hidden>{item.emoji}</span> {item.label}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
            {errors.choice && (
              <p className="text-xs text-danger" role="alert">
                {errors.choice}
              </p>
            )}
          </div>
          <Field label="Motif" error={errors.motif} hint={<CharCount value={motif} max={MOTIF_MAX} />}>
            {(props) => (
              <Textarea {...props} value={motif} onChange={(event) => setMotif(event.target.value)} rows={4} />
            )}
          </Field>
          {recrue.veto && (
            <p className="rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">
              Attention : cette recrue est sous véto de {recrue.veto.by.name}.
            </p>
          )}
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
