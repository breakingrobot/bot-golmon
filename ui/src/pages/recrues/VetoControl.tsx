// Véto d'un lead (`recrue.veto`) : poser avec un motif, ou lever.

import { Ban, Undo2 } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useClearRecrueVeto, useSetRecrueVeto } from '@/api/queries-groupe'
import type { Recrue } from '@/api/types'
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
import { MOTIF_MAX } from '@/lib/recrue'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'

export type VetoControlProps = { guildId: string; recrue: Recrue }

export const VetoControl = ({ guildId, recrue }: VetoControlProps) => {
  const [open, setOpen] = useState(false)
  const [motif, setMotif] = useState('')
  const [error, setError] = useState<string | null>(null)
  const set = useSetRecrueVeto(guildId)
  const clear = useClearRecrueVeto(guildId)

  if (recrue.veto) {
    return (
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button size="sm" variant="outline">
            <Undo2 aria-hidden /> Lever le véto
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Lever le véto sur {recrue.member.name} ?</AlertDialogTitle>
            <AlertDialogDescription>
              Véto de {recrue.veto.by.name} : « {recrue.veto.motif} ». L'action est inscrite au journal du staff.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction disabled={clear.isPending} onClick={() => clear.mutate(recrue.entryId)}>
              Lever le véto
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    )
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const text = motif.trim()
    const next = text
      ? text.length > MOTIF_MAX
        ? `${MOTIF_MAX} caractères au plus.`
        : null
      : 'Le motif est obligatoire.'
    setError(next)
    if (next) return
    set.mutate({ entryId: recrue.entryId, motif: text }, { onSuccess: () => setOpen(false) })
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) {
          setMotif('')
          setError(null)
          set.reset()
        }
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm" variant="outline" className="text-danger hover:text-danger">
          <Ban aria-hidden /> Poser un véto
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Véto sur {recrue.member.name}</DialogTitle>
          <DialogDescription>Le véto bloque la validation de la recrue. Motif obligatoire.</DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <Field label="Motif du véto" error={error} hint={<CharCount value={motif} max={MOTIF_MAX} />}>
            {(props) => (
              <Textarea {...props} value={motif} onChange={(event) => setMotif(event.target.value)} rows={4} />
            )}
          </Field>
          {set.isError && (
            <p role="alert" className="rounded-md border border-danger/40 bg-danger-soft px-3 py-2 text-sm text-danger">
              {errorMessage(set.error)}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" variant="destructive" disabled={set.isPending}>
              {set.isPending ? 'Enregistrement…' : 'Poser le véto'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
