// Inventaire d'une propriété : on saisit les quantités comptées en jeu, le bot enregistre les écarts comme
// corrections signées. Seules les lignes remplies et différentes du stock connu partent.

import { useQuery } from '@tanstack/react-query'
import { ClipboardList } from 'lucide-react'
import { useId, useState } from 'react'
import type { FormEvent, ReactElement } from 'react'

import { errorMessage } from '@/api/client'
import { coffreStockQuery, useSetCoffreInventory } from '@/api/queries'
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
import { Skeleton } from '@/atoms/Skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { propertyLabel } from '@/lib/coffre'
import {
  fallbackItemLabel,
  inventoryDiff,
  isPropertyId,
  MAX_INVENTORY,
  MAX_INVENTORY_ITEMS,
  parseQuantity,
} from '@/lib/coffre-inventory'
import type { InventoryEntry } from '@/lib/coffre-inventory'
import { formatNumber } from '@/lib/format'
import { Field } from '@/molecules/Field'
import { InventoryAddItem } from '@/molecules/InventoryAddItem'
import { InventoryCountRow } from '@/molecules/InventoryCountRow'
import { InventoryDiffList } from '@/molecules/InventoryDiffList'
import { InventoryListInput } from '@/molecules/InventoryListInput'

type Property = { id: string; name: string | null }

export type CoffreInventoryDialogProps = {
  guildId: string
  /** Propriété imposée (carte du stock) ; sinon choix parmi `properties` ou nouveau numéro. */
  property?: Property
  properties: readonly Property[]
  /** Objets connus du serveur (suggestions et noms). */
  items: readonly { item: string; label: string }[]
  trigger: ReactElement
}

type Row = { item: string; label: string; current: number | null }

const NEW = '__nouvelle__'
const QUANTITY_ERROR = `Entier entre 0 et ${formatNumber(MAX_INVENTORY)}.`

export const CoffreInventoryDialog = ({
  guildId,
  property,
  properties,
  items,
  trigger,
}: CoffreInventoryDialogProps) => {
  const [open, setOpen] = useState(false)
  const [choice, setChoice] = useState(property?.id ?? properties[0]?.id ?? NEW)
  const [typed, setTyped] = useState('')
  const [mode, setMode] = useState<'objets' | 'liste'>('objets')
  /** Quantité saisie par objet (texte brut), et objets ajoutés hors du stock connu. */
  const [draft, setDraft] = useState<Record<string, string>>({})
  const [added, setAdded] = useState<string[]>([])
  const [formError, setFormError] = useState<string | null>(null)
  const previewId = useId()
  const mutation = useSetCoffreInventory(guildId)

  const propertyId = (property?.id ?? (choice === NEW ? typed : choice)).trim()
  const validProperty = isPropertyId(propertyId)
  const stock = useQuery({ ...coffreStockQuery(guildId, propertyId), enabled: open && validProperty })
  const known = stock.data?.properties.find((entry) => entry.id === propertyId)?.items ?? []
  const labelOf = (item: string) => items.find((entry) => entry.item === item)?.label ?? fallbackItemLabel(item)
  const rows: Row[] = [
    ...known.map((entry) => ({ item: entry.item, label: entry.label, current: entry.quantity })),
    ...added
      .filter((item) => !known.some((entry) => entry.item === item))
      .map((item) => ({ item, label: labelOf(item), current: null })),
  ]
  const counted: InventoryEntry[] = []
  const invalid = new Set<string>()
  for (const row of rows) {
    const text = draft[row.item]?.trim() ?? ''
    if (!text) continue
    const quantity = parseQuantity(text)
    if (quantity === null) invalid.add(row.item)
    else counted.push({ item: row.item, quantity })
  }
  const diffs = inventoryDiff(known, counted, labelOf)
  const unchanged = counted.length - diffs.length

  const reset = () => {
    setDraft({})
    setAdded([])
    setFormError(null)
    mutation.reset()
  }

  const fill = (entries: InventoryEntry[]) => {
    setDraft((prev) => ({
      ...prev,
      ...Object.fromEntries(entries.map((entry) => [entry.item, String(entry.quantity)])),
    }))
    setAdded((prev) => [
      ...prev,
      ...entries
        .map((entry) => entry.item)
        .filter((item) => !prev.includes(item) && !known.some((entry) => entry.item === item)),
    ])
    setFormError(null)
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (!validProperty) setFormError('Choisissez une propriété ou saisissez son numéro (ex : 8740).')
    else if (invalid.size > 0) setFormError('Corrigez les quantités signalées.')
    else if (diffs.length === 0) setFormError('Aucun écart avec le stock connu : rien à corriger.')
    else if (diffs.length > MAX_INVENTORY_ITEMS) setFormError(`${MAX_INVENTORY_ITEMS} objets au plus par inventaire.`)
    else {
      setFormError(null)
      mutation.mutate(
        { propertyId, items: diffs.map((diff) => ({ item: diff.item, quantity: diff.after })) },
        { onSuccess: () => setOpen(false) },
      )
    }
  }

  const error = formError ?? (mutation.isError ? errorMessage(mutation.error) : null)

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) {
          reset()
          setChoice(property?.id ?? properties[0]?.id ?? NEW)
          setTyped('')
          setMode('objets')
        }
      }}
    >
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ClipboardList className="size-5 text-brand" aria-hidden />
            {property ? `Inventaire : ${propertyLabel(property)}` : 'Inventaire'}
          </DialogTitle>
          <DialogDescription>
            Comptez en jeu et saisissez la quantité réelle : l’écart avec le stock connu devient une correction. Les
            lignes laissées vides ne changent rien.
          </DialogDescription>
        </DialogHeader>
        <form className="grid min-w-0 gap-5" onSubmit={submit} noValidate>
          {!property && (
            <div className="grid gap-3 sm:grid-cols-2">
              <Field label="Propriété">
                {(props) => (
                  <Select
                    value={choice}
                    onValueChange={(value) => {
                      setChoice(value)
                      reset()
                    }}
                  >
                    <SelectTrigger {...props}>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {properties.map((entry) => (
                        <SelectItem key={entry.id} value={entry.id}>
                          {propertyLabel(entry)}
                        </SelectItem>
                      ))}
                      <SelectItem value={NEW}>Autre numéro…</SelectItem>
                    </SelectContent>
                  </Select>
                )}
              </Field>
              {choice === NEW && (
                <Field label="Numéro de la propriété" hint="Le numéro du jeu, ex : 8740.">
                  {(props) => (
                    <Input
                      {...props}
                      inputMode="numeric"
                      value={typed}
                      onChange={(event) => {
                        setTyped(event.target.value)
                        reset()
                      }}
                      placeholder="8740"
                      autoFocus
                    />
                  )}
                </Field>
              )}
            </div>
          )}

          <ToggleGroup
            type="single"
            value={mode}
            onValueChange={(value) => (value === 'objets' || value === 'liste') && setMode(value)}
            aria-label="Mode de saisie"
          >
            <ToggleGroupItem value="objets">Objet par objet</ToggleGroupItem>
            <ToggleGroupItem value="liste">Coller une liste</ToggleGroupItem>
          </ToggleGroup>

          <div className="grid min-w-0 gap-5 md:grid-cols-[minmax(0,1fr)_18rem]">
            {mode === 'liste' ? (
              <InventoryListInput
                onFill={(entries) => {
                  fill(entries)
                  setMode('objets')
                }}
              />
            ) : validProperty ? (
              <div className="grid min-w-0 content-start gap-3">
                {stock.isPending ? (
                  <Skeleton className="h-32 rounded-md" />
                ) : stock.isError ? (
                  <p className="text-sm text-danger">Stock connu indisponible : {errorMessage(stock.error)}</p>
                ) : (
                  <fieldset className="grid min-w-0">
                    <legend className="mb-1 flex w-full justify-between text-xs font-medium tracking-wide text-ink-subtle uppercase">
                      <span>Objet · stock connu</span>
                      <span className="pr-10">Compté</span>
                    </legend>
                    {rows.length === 0 && (
                      <p className="py-2 text-sm text-ink-muted">Aucun objet connu : ajoutez-les ci-dessous.</p>
                    )}
                    {rows.map((row) => (
                      <InventoryCountRow
                        key={row.item}
                        label={row.label}
                        current={row.current}
                        value={draft[row.item] ?? ''}
                        onChange={(value) => setDraft((prev) => ({ ...prev, [row.item]: value }))}
                        error={invalid.has(row.item) ? QUANTITY_ERROR : null}
                        onRemove={
                          row.current === null
                            ? () => setAdded((prev) => prev.filter((item) => item !== row.item))
                            : undefined
                        }
                      />
                    ))}
                  </fieldset>
                )}
                <InventoryAddItem items={items} onAdd={(entry) => fill([entry])} />
              </div>
            ) : (
              <p className="text-sm text-ink-muted">Choisissez d’abord la propriété.</p>
            )}

            <section
              aria-labelledby={previewId}
              className="grid min-w-0 content-start gap-2 rounded-md bg-surface-sunken p-3"
            >
              <h3 id={previewId} className="text-sm font-medium text-ink">
                Aperçu des corrections
              </h3>
              <p className="text-xs text-ink-muted" aria-live="polite">
                {diffs.length === 0
                  ? 'Aucun écart pour l’instant.'
                  : `${diffs.length} correction(s) à enregistrer${unchanged > 0 ? `, ${unchanged} objet(s) déjà juste(s)` : ''}.`}
              </p>
              <InventoryDiffList diffs={diffs} />
            </section>
          </div>

          {error && (
            <p className="rounded-md bg-danger-soft px-3 py-2 text-sm text-danger" role="alert">
              {error}
            </p>
          )}

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Annuler
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {diffs.length > 0 ? `Enregistrer ${diffs.length} correction(s)` : 'Enregistrer'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
