import { Trash2 } from 'lucide-react'
import { useId } from 'react'

import { Button } from '@/atoms/Button'
import { Input } from '@/atoms/Input'
import { formatNumber } from '@/lib/format'
import { cn } from '@/lib/utils'

export type InventoryCountRowProps = {
  label: string
  /** Quantité connue ; `null` pour un objet ajouté, absent du stock. */
  current: number | null
  value: string
  onChange: (value: string) => void
  error?: string | null
  /** Retirer la ligne (objets ajoutés seulement). */
  onRemove?: (() => void) | undefined
}

/** Une ligne d'inventaire : objet, stock connu et quantité comptée (vide : non modifiée). */
export const InventoryCountRow = ({ label, current, value, onChange, error, onRemove }: InventoryCountRowProps) => {
  const id = useId()
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_6.5rem_2rem] items-center gap-2 border-b py-1.5 last:border-b-0">
      <label htmlFor={id} className="min-w-0 text-sm">
        <span className="block truncate font-medium text-ink">{label}</span>
        <span className="block text-xs text-ink-subtle tabular-nums">
          {current === null ? (
            'nouvel objet'
          ) : (
            <>
              connu : <span className={cn(current < 0 && 'text-danger')}>{formatNumber(current)}</span>
            </>
          )}
        </span>
      </label>
      <Input
        id={id}
        inputMode="numeric"
        className="text-right tabular-nums"
        value={value}
        placeholder={current === null ? '0' : String(current)}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? `${id}-erreur` : undefined}
        onChange={(event) => onChange(event.target.value)}
      />
      {onRemove ? (
        <Button type="button" variant="ghost" size="icon-sm" aria-label={`Retirer ${label}`} onClick={onRemove}>
          <Trash2 aria-hidden />
        </Button>
      ) : (
        <span aria-hidden />
      )}
      {error && (
        <p id={`${id}-erreur`} className="col-span-3 text-xs text-danger">
          {error}
        </p>
      )}
    </div>
  )
}
