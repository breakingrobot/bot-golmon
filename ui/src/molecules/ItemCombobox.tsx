// Champ d'objet des coffres : suggestions parmi les objets connus, texte libre admis (nouvel objet du jeu).

import { useId, useState } from 'react'
import type { KeyboardEvent } from 'react'

import { Input } from '@/atoms/Input'
import { cn } from '@/lib/utils'

export type ItemComboboxProps = {
  id?: string
  value: string
  onChange: (value: string) => void
  /** Objets connus : identifiant du jeu et nom affiché. */
  items: readonly { item: string; label: string }[]
  placeholder?: string
  invalid?: boolean
  'aria-describedby'?: string
  'aria-invalid'?: boolean
  /** Entrée sans suggestion surlignée (par défaut : laissée au formulaire). */
  onEnter?: () => void
}

const MAX_SUGGESTIONS = 8

export const ItemCombobox = ({
  id,
  value,
  onChange,
  items,
  placeholder = 'repair_kit',
  invalid,
  'aria-describedby': describedBy,
  'aria-invalid': ariaInvalid,
  onEnter,
}: ItemComboboxProps) => {
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)
  const listId = useId()
  const query = value.trim().toLowerCase()
  const suggestions = items
    .filter(
      (entry) => !query || entry.item.includes(query.replaceAll(' ', '_')) || entry.label.toLowerCase().includes(query),
    )
    .slice(0, MAX_SUGGESTIONS)
  const expanded = open && suggestions.length > 0
  const activeEntry = expanded ? suggestions[active] : undefined

  const choose = (item: string) => {
    onChange(item)
    setOpen(false)
    setActive(-1)
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setOpen(true)
      setActive((index) => Math.min(index + 1, suggestions.length - 1))
    } else if (event.key === 'ArrowUp') {
      event.preventDefault()
      setActive((index) => Math.max(index - 1, 0))
    } else if (event.key === 'Enter' && activeEntry) {
      // Entrée choisit la suggestion surlignée.
      event.preventDefault()
      choose(activeEntry.item)
    } else if (event.key === 'Enter' && onEnter) {
      event.preventDefault()
      onEnter()
    } else if (event.key === 'Escape' && expanded) {
      // Ferme la liste sans fermer le dialogue.
      event.preventDefault()
      event.stopPropagation()
      setOpen(false)
    }
  }

  return (
    <div className="relative">
      <Input
        id={id}
        role="combobox"
        autoComplete="off"
        spellCheck={false}
        aria-autocomplete="list"
        aria-expanded={expanded}
        aria-controls={listId}
        aria-activedescendant={activeEntry ? `${listId}-${activeEntry.item}` : undefined}
        aria-invalid={invalid || ariaInvalid}
        aria-describedby={describedBy}
        value={value}
        placeholder={placeholder}
        onChange={(event) => {
          onChange(event.target.value)
          setOpen(true)
          setActive(-1)
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={onKeyDown}
      />
      <ul
        id={listId}
        role="listbox"
        aria-label="Objets connus"
        hidden={!expanded}
        className="absolute inset-x-0 top-full z-10 mt-1 max-h-56 overflow-y-auto rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
      >
        {suggestions.map((entry, index) => (
          <li
            key={entry.item}
            id={`${listId}-${entry.item}`}
            role="option"
            aria-selected={index === active}
            tabIndex={-1}
            onMouseEnter={() => setActive(index)}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => choose(entry.item)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') choose(entry.item)
            }}
            className={cn(
              'flex cursor-pointer items-baseline justify-between gap-3 rounded-sm px-2 py-1.5 text-sm',
              index === active && 'bg-accent',
            )}
          >
            <span className="truncate">{entry.label}</span>
            <span className="shrink-0 font-mono text-xs text-ink-subtle">{entry.item}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
