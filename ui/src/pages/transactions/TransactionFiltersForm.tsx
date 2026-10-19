import { Filter, X } from 'lucide-react'
import { useState } from 'react'

import type { TransactionFilters } from '@/api/queries'
import type { TransactionStatus } from '@/api/types'
import { Button } from '@/atoms/Button'
import { Input } from '@/atoms/Input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { ALL, compact, fromSelect } from '@/lib/search'
import { Field } from '@/molecules/Field'
import { FilterBar } from '@/molecules/FilterBar'

export type TransactionFiltersFormProps = {
  initial: TransactionFilters
  statuses: { value: TransactionStatus; label: string }[]
  onApply: (filters: TransactionFilters) => void
  onReset: (() => void) | undefined
}

export const TransactionFiltersForm = ({ initial, statuses, onApply, onReset }: TransactionFiltersFormProps) => {
  const [groupe, setGroupe] = useState(initial.groupe ?? '')
  const [statut, setStatut] = useState(initial.statut ?? ALL)
  const [type, setType] = useState(initial.type ?? ALL)
  const [du, setDu] = useState(initial.du ?? '')
  const [au, setAu] = useState(initial.au ?? '')
  return (
    <FilterBar
      onSubmit={() =>
        onApply(
          compact({
            groupe: groupe.trim() || undefined,
            statut: fromSelect(statut),
            type: fromSelect(type),
            du: du || undefined,
            au: au || undefined,
          }),
        )
      }
    >
      <Field label="Groupe">
        {(props) => (
          <Input
            {...props}
            type="search"
            value={groupe}
            onChange={(event) => setGroupe(event.target.value)}
            placeholder="Nom (partiel)"
          />
        )}
      </Field>
      <Field label="Statut">
        {(props) => (
          <Select value={statut} onValueChange={setStatut}>
            <SelectTrigger {...props}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL}>Tous les statuts</SelectItem>
              {statuses.map((status) => (
                <SelectItem key={status.value} value={status.value}>
                  {status.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </Field>
      <Field label="Type">
        {(props) => (
          <Select value={type} onValueChange={setType}>
            <SelectTrigger {...props}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL}>Achats et ventes</SelectItem>
              <SelectItem value="ACHAT">Achats</SelectItem>
              <SelectItem value="VENTE">Ventes</SelectItem>
            </SelectContent>
          </Select>
        )}
      </Field>
      <Field label="Du">
        {(props) => (
          <Input
            {...props}
            type="date"
            value={du}
            max={au || undefined}
            onChange={(event) => setDu(event.target.value)}
          />
        )}
      </Field>
      <Field label="Au">
        {(props) => (
          <Input
            {...props}
            type="date"
            value={au}
            min={du || undefined}
            onChange={(event) => setAu(event.target.value)}
          />
        )}
      </Field>
      <div className="flex items-end gap-2">
        <Button type="submit">
          <Filter aria-hidden /> Filtrer
        </Button>
        {onReset && (
          <Button type="button" variant="ghost" onClick={onReset}>
            <X aria-hidden /> Réinitialiser
          </Button>
        )}
      </div>
    </FilterBar>
  )
}
