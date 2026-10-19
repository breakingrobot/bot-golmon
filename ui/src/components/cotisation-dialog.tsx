// Dialogue « Ajouter une cotisation » : pour soi, ou pour un membre avec `cotisation.gerer`.

import { Plus } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useAddCotisation } from '@/api/queries'
import type { MemberRef, NewCotisation } from '@/api/types'
import { Field } from '@/components/form'
import { MemberPicker } from '@/components/member-picker'
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
import { Input, Textarea } from '@/components/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatMoney, todayParis } from '@/lib/format'

function parseAmount(text: string): number | null {
  const cleaned = text.replace(/[\s  $]/g, '')
  if (cleaned === '') return 0
  if (!/^\d+$/.test(cleaned)) return null
  return Number(cleaned)
}

export function AddCotisationDialog({
  guildId,
  canManage,
  fixedMember,
  triggerLabel = 'Ajouter une cotisation',
}: {
  guildId: string
  /** `cotisation.gerer` : déclarer pour un autre membre. */
  canManage: boolean
  /** Membre imposé (fiche d'un membre). */
  fixedMember?: MemberRef | null
  triggerLabel?: string
}) {
  const [open, setOpen] = useState(false)
  const [forOther, setForOther] = useState(Boolean(fixedMember))
  const [target, setTarget] = useState<MemberRef | null>(fixedMember ?? null)
  const [propre, setPropre] = useState('')
  const [sale, setSale] = useState('')
  const [description, setDescription] = useState('')
  const [date, setDate] = useState(todayParis())
  const [errors, setErrors] = useState<Record<string, string>>({})
  const mutation = useAddCotisation(guildId)

  const reset = () => {
    setForOther(Boolean(fixedMember))
    setTarget(fixedMember ?? null)
    setPropre('')
    setSale('')
    setDescription('')
    setDate(todayParis())
    setErrors({})
    mutation.reset()
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: Record<string, string> = {}
    const propreValue = parseAmount(propre)
    const saleValue = parseAmount(sale)
    if (propreValue === null) next.propre = 'Montant entier en dollars, sans décimales.'
    if (saleValue === null) next.sale = 'Montant entier en dollars, sans décimales.'
    if (propreValue !== null && saleValue !== null && propreValue + saleValue <= 0) {
      next.propre = 'Indiquez au moins un montant propre ou sale.'
    }
    if (forOther && !target) next.member = 'Choisissez le membre.'
    if (date > todayParis()) next.date = 'La date ne peut pas être dans le futur.'
    setErrors(next)
    if (Object.keys(next).length > 0 || propreValue === null || saleValue === null) return
    const body: NewCotisation = {
      propre: propreValue,
      sale: saleValue,
      description: description.trim(),
      ...(date && date !== todayParis() ? { date } : {}),
      ...(forOther && target ? { memberId: target.id } : {}),
    }
    mutation.mutate(body, {
      onSuccess: () => {
        setOpen(false)
        reset()
      },
    })
  }

  const total = (parseAmount(propre) ?? 0) + (parseAmount(sale) ?? 0)

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
          <Plus aria-hidden /> {triggerLabel}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Ajouter une cotisation</DialogTitle>
          <DialogDescription>Montants en dollars du jeu. La cotisation apparaît aussi sur Discord.</DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          {canManage && !fixedMember && (
            <div className="grid gap-2">
              <span className="text-sm font-medium text-ink-muted" id="cotisation-pour">
                Pour
              </span>
              <ToggleGroup
                type="single"
                value={forOther ? 'autre' : 'moi'}
                onValueChange={(value) => value && setForOther(value === 'autre')}
                aria-labelledby="cotisation-pour"
              >
                <ToggleGroupItem value="moi">Moi-même</ToggleGroupItem>
                <ToggleGroupItem value="autre">Un autre membre</ToggleGroupItem>
              </ToggleGroup>
              {forOther && (
                <Field label="Membre" error={errors.member}>
                  {(props) => (
                    <MemberPicker
                      guildId={guildId}
                      value={target}
                      onChange={setTarget}
                      id={props.id}
                      invalid={Boolean(errors.member)}
                    />
                  )}
                </Field>
              )}
            </div>
          )}
          {fixedMember && (
            <p className="text-sm text-ink-muted">
              Pour <span className="font-medium text-ink">{fixedMember.name}</span>
            </p>
          )}
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Argent propre ($)" error={errors.propre}>
              {(props) => (
                <Input
                  {...props}
                  inputMode="numeric"
                  value={propre}
                  onChange={(event) => setPropre(event.target.value)}
                  placeholder="0"
                  autoComplete="off"
                />
              )}
            </Field>
            <Field label="Argent sale ($)" error={errors.sale}>
              {(props) => (
                <Input
                  {...props}
                  inputMode="numeric"
                  value={sale}
                  onChange={(event) => setSale(event.target.value)}
                  placeholder="0"
                  autoComplete="off"
                />
              )}
            </Field>
          </div>
          <Field label="Description" hint="Facultatif : d'où vient l'argent.">
            {(props) => (
              <Textarea
                {...props}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                rows={2}
              />
            )}
          </Field>
          <Field label="Date" error={errors.date} hint="Aujourd'hui par défaut.">
            {(props) => (
              <Input
                {...props}
                type="date"
                value={date}
                max={todayParis()}
                onChange={(event) => setDate(event.target.value)}
              />
            )}
          </Field>
          {/* Refus du serveur (par exemple : la personne n'a pas le rôle membre), en plus de la notification. */}
          {mutation.isError && (
            <p role="alert" className="rounded-md border border-danger/40 bg-danger-soft px-3 py-2 text-sm text-danger">
              {errorMessage(mutation.error)}
            </p>
          )}
          <DialogFooter className="items-center">
            {total > 0 && <span className="mr-auto text-sm text-ink-muted">Total : {formatMoney(total)}</span>}
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
