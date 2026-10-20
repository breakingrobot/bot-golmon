import { useState } from 'react'

import { Button } from '@/atoms/Button'
import { Textarea } from '@/atoms/Textarea'
import { parseInventoryList } from '@/lib/coffre-inventory'
import type { InventoryEntry } from '@/lib/coffre-inventory'
import { Field } from '@/molecules/Field'

export type InventoryListInputProps = {
  /** Entrées lues, à reporter dans les lignes de l'inventaire. */
  onFill: (entries: InventoryEntry[]) => void
}

/** « Coller une liste » : même grammaire que la commande Discord (`repair_kit:42, acetone 2790; x=1`). */
export const InventoryListInput = ({ onFill }: InventoryListInputProps) => {
  const [text, setText] = useState('')
  const [error, setError] = useState<string | null>(null)
  const read = () => {
    const parsed = parseInventoryList(text)
    if (parsed.ok) {
      setText('')
      setError(null)
      onFill(parsed.entries)
    } else setError(parsed.error)
  }
  return (
    <div className="grid content-start gap-2">
      <Field
        label="Liste objet:quantité"
        hint="Séparées par des virgules, des points-virgules ou des lignes : repair_kit:42, acetone 2790; x=1"
        error={error}
      >
        {(props) => (
          <Textarea
            {...props}
            rows={6}
            className="font-mono"
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder={'repair_kit:42\nacetone:2790'}
            autoFocus
          />
        )}
      </Field>
      <Button type="button" variant="outline" className="justify-self-start" onClick={read}>
        Remplir les lignes
      </Button>
    </div>
  )
}
