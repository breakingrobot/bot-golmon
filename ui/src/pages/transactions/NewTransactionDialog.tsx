// Dialogue « Nouvelle transaction » (`transaction.utiliser`) : la fiche est publiée dans le salon du tableau de bord.

import { Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { errorMessage } from '@/api/client'
import { useCreateTransaction } from '@/api/queries-groupe'
import type { NewTransaction, TransactionItemInput } from '@/api/types'
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
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { CharCount } from '@/molecules/CharCount'
import { Field } from '@/molecules/Field'

/** Prix saisi (« 1 500 », « 12,5 ») ; `null` si invalide. */
export const parsePrice = (text: string): number | null => {
  const cleaned = text.replaceAll(/[\s  $]/gu, '').replace(',', '.')
  if (!/^\d+(?<cents>\.\d{1,2})?$/u.test(cleaned)) return null
  return Number(cleaned)
}

/** Longueur maximale de la note (comme sur Discord). */
const NOTE_MAX = 500

export type ItemRow = { key: number; nom: string; quantite: string; prix: string }

let nextKey = 0
const emptyItem = (): ItemRow => {
  nextKey += 1
  return { key: nextKey, nom: '', quantite: '1', prix: '' }
}

/** Objets saisis convertis pour l'API ; `null` : ligne invalide. Les lignes sans nom sont ignorées. */
export const itemsOf = (rows: ItemRow[]): TransactionItemInput[] | null => {
  const items: TransactionItemInput[] = []
  for (const row of rows) {
    if (!row.nom.trim()) continue
    const quantite = Number(row.quantite || '1')
    if (!Number.isInteger(quantite) || quantite < 1) return null
    const prix = row.prix.trim() ? parsePrice(row.prix) : undefined
    if (prix === null) return null
    items.push({ nom: row.nom.trim(), quantite, ...(prix === undefined ? {} : { prixUnitaire: prix }) })
  }
  return items
}

export type NewTransactionDialogProps = { guildId: string }

export const NewTransactionDialog = ({ guildId }: NewTransactionDialogProps) => {
  const [open, setOpen] = useState(false)
  const [groupe, setGroupe] = useState('')
  const [type, setType] = useState<'ACHAT' | 'VENTE'>('VENTE')
  const [prix, setPrix] = useState('')
  const [rows, setRows] = useState<ItemRow[]>([])
  const [note, setNote] = useState('')
  const [errors, setErrors] = useState<Partial<Record<'groupe' | 'prix' | 'items', string>>>({})
  const mutation = useCreateTransaction(guildId)

  const reset = () => {
    setGroupe('')
    setType('VENTE')
    setPrix('')
    setRows([])
    setNote('')
    setErrors({})
    mutation.reset()
  }

  const update = (key: number, patch: Partial<ItemRow>) =>
    setRows((current) => current.map((row) => (row.key === key ? { ...row, ...patch } : row)))

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const next: typeof errors = {}
    const price = parsePrice(prix)
    const items = itemsOf(rows)
    if (!groupe.trim()) next.groupe = 'Indiquez le groupe concerné.'
    else if (groupe.trim().length > 100) next.groupe = '100 caractères au plus.'
    if (price === null) next.prix = 'Montant en dollars, deux décimales au plus.'
    if (items === null) next.items = 'Quantité entière (1 au moins) et prix unitaire valide pour chaque objet.'
    setErrors(next)
    if (Object.keys(next).length > 0 || price === null || items === null) return
    const body: NewTransaction = {
      groupe: groupe.trim(),
      type,
      prix: price,
      ...(items.length > 0 ? { items } : {}),
      ...(note.trim() ? { note: note.trim() } : {}),
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
          <Plus aria-hidden /> Nouvelle transaction
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Nouvelle transaction</DialogTitle>
          <DialogDescription>
            La fiche est publiée dans le salon du tableau de bord des transactions (le rôle configuré est mentionné), et
            l'action inscrite au journal.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <Field label="Groupe" error={errors.groupe}>
            {(props) => (
              <Input {...props} value={groupe} onChange={(event) => setGroupe(event.target.value)} autoFocus />
            )}
          </Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <span className="text-sm font-medium text-ink-muted" id="transaction-type">
                Type
              </span>
              <ToggleGroup
                type="single"
                value={type}
                onValueChange={(value) => value && setType(value as 'ACHAT' | 'VENTE')}
                aria-labelledby="transaction-type"
              >
                <ToggleGroupItem value="VENTE">Vente</ToggleGroupItem>
                <ToggleGroupItem value="ACHAT">Achat</ToggleGroupItem>
              </ToggleGroup>
            </div>
            <Field label="Prix total ($)" error={errors.prix}>
              {(props) => (
                <Input
                  {...props}
                  inputMode="decimal"
                  value={prix}
                  onChange={(event) => setPrix(event.target.value)}
                  placeholder="1500"
                />
              )}
            </Field>
          </div>
          <fieldset className="grid gap-2">
            <legend className="text-sm font-medium text-ink-muted">Objets (facultatif)</legend>
            {rows.map((row, index) => (
              <div key={row.key} className="grid grid-cols-[1fr_4.5rem_auto] gap-2 sm:grid-cols-[1fr_4.5rem_7rem_auto]">
                <Input
                  value={row.nom}
                  onChange={(event) => update(row.key, { nom: event.target.value })}
                  placeholder="Objet"
                  aria-label={`Nom de l'objet ${index + 1}`}
                />
                <Input
                  value={row.quantite}
                  inputMode="numeric"
                  onChange={(event) => update(row.key, { quantite: event.target.value })}
                  aria-label={`Quantité de l'objet ${index + 1}`}
                />
                <Input
                  value={row.prix}
                  inputMode="decimal"
                  onChange={(event) => update(row.key, { prix: event.target.value })}
                  placeholder="Prix / u"
                  aria-label={`Prix unitaire de l'objet ${index + 1}`}
                  className="col-span-2 sm:col-span-1"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={`Retirer l'objet ${index + 1}`}
                  onClick={() => setRows((current) => current.filter((item) => item.key !== row.key))}
                  className="col-start-3 row-start-1 sm:col-start-auto"
                >
                  <Trash2 aria-hidden />
                </Button>
              </div>
            ))}
            {errors.items && (
              <p className="text-xs text-danger" role="alert">
                {errors.items}
              </p>
            )}
            {rows.length < 25 && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="w-fit"
                onClick={() => setRows((current) => [...current, emptyItem()])}
              >
                <Plus aria-hidden /> Ajouter un objet
              </Button>
            )}
          </fieldset>
          <Field label="Note (facultatif)">
            {(props) => (
              <>
                <Textarea
                  {...props}
                  value={note}
                  maxLength={NOTE_MAX}
                  rows={2}
                  onChange={(event) => setNote(event.target.value)}
                  placeholder="RDV au port, paiement en deux fois…"
                />
                <CharCount value={note} max={NOTE_MAX} />
              </>
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
              {mutation.isPending ? 'Publication…' : 'Créer'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
