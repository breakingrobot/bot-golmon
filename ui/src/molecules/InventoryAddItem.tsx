import { Plus } from 'lucide-react'
import { useState } from 'react'

import { Button } from '@/atoms/Button'
import { Input } from '@/atoms/Input'
import { MAX_INVENTORY, normalizeItem, parseQuantity } from '@/lib/coffre-inventory'
import type { InventoryEntry } from '@/lib/coffre-inventory'
import { formatNumber } from '@/lib/format'
import { Field } from '@/molecules/Field'
import { ItemCombobox } from '@/molecules/ItemCombobox'

export type InventoryAddItemProps = {
  /** Objets connus (suggestions) ; le texte libre est normalisé comme par le bot. */
  items: readonly { item: string; label: string }[]
  onAdd: (entry: InventoryEntry) => void
}

/** Ligne « Ajouter un objet » d'un inventaire : objet (suggestions ou texte libre) et quantité comptée. */
export const InventoryAddItem = ({ items, onAdd }: InventoryAddItemProps) => {
  const [item, setItem] = useState('')
  const [quantity, setQuantity] = useState('')
  const [error, setError] = useState<string | null>(null)
  const add = () => {
    const normalized = normalizeItem(item)
    const counted = parseQuantity(quantity)
    if (!normalized) setError('Nom d’objet invalide : minuscules, chiffres, _ - . (ex : repair_kit).')
    else if (counted === null) setError(`Quantité : entier entre 0 et ${formatNumber(MAX_INVENTORY)}.`)
    else {
      onAdd({ item: normalized, quantity: counted })
      setItem('')
      setQuantity('')
      setError(null)
    }
  }
  return (
    <div className="grid gap-2 rounded-md border border-dashed p-3">
      <p className="text-sm font-medium text-ink">Ajouter un objet</p>
      <div className="grid grid-cols-[minmax(0,1fr)_6.5rem_auto] items-end gap-2">
        <Field label="Objet">
          {(props) => (
            <ItemCombobox
              {...props}
              value={item}
              onChange={setItem}
              items={items}
              invalid={error !== null}
              onEnter={add}
            />
          )}
        </Field>
        <Field label="Quantité">
          {(props) => (
            <Input
              {...props}
              inputMode="numeric"
              className="text-right tabular-nums"
              value={quantity}
              onChange={(event) => setQuantity(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault()
                  add()
                }
              }}
              placeholder="0"
            />
          )}
        </Field>
        <Button type="button" variant="outline" size="icon" aria-label="Ajouter l’objet" onClick={add}>
          <Plus aria-hidden />
        </Button>
      </div>
      {error && (
        <p className="text-xs text-danger" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}
