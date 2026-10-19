// Sélecteur de membre : recherche dans les membres en cache du bot (GET /members?q=).

import { useQuery } from '@tanstack/react-query'
import { ChevronsUpDown, Search } from 'lucide-react'
import { useId, useState, type KeyboardEvent } from 'react'

import { memberSearchQuery } from '@/api/queries'
import type { MemberRef } from '@/api/types'
import { MemberAvatar } from '@/components/member'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useDebounced } from '@/hooks/use-debounced'
import { cn } from '@/lib/utils'

export function MemberPicker({
  guildId,
  value,
  onChange,
  placeholder = 'Choisir un membre',
  id,
  invalid,
}: {
  guildId: string
  value: MemberRef | null
  onChange: (member: MemberRef | null) => void
  placeholder?: string
  id?: string
  invalid?: boolean
}) {
  const [open, setOpen] = useState(false)
  const [text, setText] = useState('')
  const [active, setActive] = useState(0)
  const q = useDebounced(text.trim())
  const listId = useId()
  const search = useQuery({ ...memberSearchQuery(guildId, q), enabled: open })
  const results = search.data ?? []

  const choose = (member: MemberRef) => {
    onChange(member)
    setOpen(false)
    setText('')
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setActive((index) => Math.min(index + 1, results.length - 1))
    } else if (event.key === 'ArrowUp') {
      event.preventDefault()
      setActive((index) => Math.max(index - 1, 0))
    } else if (event.key === 'Enter') {
      event.preventDefault()
      const member = results[active]
      if (member) choose(member)
    }
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          variant="outline"
          role="combobox"
          aria-expanded={open}
          aria-controls={listId}
          aria-invalid={invalid}
          className="w-full justify-between px-3 font-normal"
        >
          {value ? (
            <span className="flex min-w-0 items-center gap-2">
              <MemberAvatar member={value} className="size-5" />
              <span className="truncate">{value.name}</span>
            </span>
          ) : (
            <span className="text-ink-subtle">{placeholder}</span>
          )}
          <ChevronsUpDown className="opacity-50" aria-hidden />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="p-0">
        <div className="flex items-center gap-2 border-b px-3">
          <Search className="size-4 text-ink-subtle" aria-hidden />
          <input
            autoFocus
            value={text}
            onChange={(event) => {
              setText(event.target.value)
              setActive(0)
            }}
            onKeyDown={onKeyDown}
            placeholder="Rechercher un membre…"
            aria-label="Rechercher un membre"
            aria-controls={listId}
            aria-activedescendant={results[active] ? `${listId}-${results[active].id}` : undefined}
            className="h-10 w-full bg-transparent text-sm outline-none placeholder:text-ink-subtle"
          />
        </div>
        <ul id={listId} role="listbox" aria-label="Membres" className="max-h-64 overflow-y-auto p-1">
          {search.isPending && <li className="px-2 py-3 text-sm text-ink-muted">Recherche…</li>}
          {search.isError && <li className="px-2 py-3 text-sm text-danger">Recherche impossible.</li>}
          {search.isSuccess && results.length === 0 && (
            <li className="px-2 py-3 text-sm text-ink-muted">Aucun membre trouvé.</li>
          )}
          {results.map((member, index) => (
            <li
              key={member.id}
              id={`${listId}-${member.id}`}
              role="option"
              aria-selected={value?.id === member.id}
              tabIndex={-1}
              onMouseEnter={() => setActive(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => choose(member)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') choose(member)
              }}
              className={cn(
                'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm',
                index === active && 'bg-accent',
              )}
            >
              <MemberAvatar member={member} className="size-6" />
              <span className="truncate">{member.name}</span>
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  )
}
