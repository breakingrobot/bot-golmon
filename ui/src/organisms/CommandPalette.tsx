// Palette de commandes : Ctrl+K / ⌘K, ou le bouton de recherche de l'en-tête. Dialogue Radix (focus
// piégé, Échap, retour du focus), champ en combobox ARIA avec liste de suggestions groupées :
// flèches haut / bas, Début / Fin, Entrée ; plein écran sur téléphone.

import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams } from '@tanstack/react-router'
import { useAtom, useSetAtom } from 'jotai'
import { CornerDownLeft, Search } from 'lucide-react'
import { useEffect, useId, useState } from 'react'
import type { KeyboardEvent } from 'react'

import { coffreMetaQuery, contactsQuery, memberSearchQuery, meQuery } from '@/api/queries'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/atoms/Dialog'
import { useDebounced } from '@/hooks/use-debounced'
import { findGuild, hasPerm } from '@/lib/guild'
import { allCommands, filterCommands } from '@/lib/palette'
import type { Command } from '@/lib/palette'
import { cn } from '@/lib/utils'
import { paletteOpenAtom, quickCotisationOpenAtom } from '@/state/palette'
import { useTheme } from '@/state/theme'

/** Ctrl+K (⌘K sur Mac) ouvre ou referme la palette, où que soit le focus. */
const isPaletteShortcut = (event: globalThis.KeyboardEvent) =>
  (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'k'

export const CommandPalette = () => {
  const [open, setOpen] = useAtom(paletteOpenAtom)
  const setCotisationOpen = useSetAtom(quickCotisationOpenAtom)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const debounced = useDebounced(query.trim(), 250)
  const navigate = useNavigate()
  const { resolved, setChoice } = useTheme()
  const listId = useId()
  const params: { guildId?: string } = useParams({ strict: false })
  const me = useQuery(meQuery)
  const guild = params.guildId ? findGuild(me.data, params.guildId) : undefined
  const guildId = guild?.id ?? ''

  const members = useQuery({
    ...memberSearchQuery(guildId, debounced),
    enabled: open && Boolean(guild) && hasPerm(guild, 'membre.fiche') && debounced.length >= 2,
  })
  const contacts = useQuery({
    ...contactsQuery(guildId, {}),
    enabled: open && Boolean(guild) && hasPerm(guild, 'contact.voir'),
  })
  const coffres = useQuery({
    ...coffreMetaQuery(guildId),
    enabled: open && Boolean(guild) && hasPerm(guild, 'coffre.voir'),
  })

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (!isPaletteShortcut(event)) return
      event.preventDefault()
      setOpen((current) => !current)
    }
    globalThis.addEventListener('keydown', onKey)
    return () => globalThis.removeEventListener('keydown', onKey)
  }, [setOpen])

  const sections = filterCommands(
    allCommands({
      me: me.data,
      guild,
      members: debounced.length >= 2 ? (members.data ?? []) : [],
      contacts: contacts.data?.items ?? [],
      coffres: coffres.data,
    }),
    query,
  )
  const flat = sections.flatMap((section) => section.commands)
  const current = Math.min(active, Math.max(0, flat.length - 1))
  const optionId = (index: number) => `${listId}-${index}`

  const close = () => {
    setOpen(false)
    setQuery('')
    setActive(0)
  }

  const run = (command: Command) => {
    close()
    const { target } = command
    switch (target.kind) {
      case 'page': {
        if (target.page.to === null) {
          const userId = me.data?.user?.id ?? ''
          void navigate({
            to: '/serveurs/$guildId/membres/$userId',
            params: { guildId: target.guildId, userId },
          })
        } else void navigate({ to: target.page.to, params: { guildId: target.guildId } })
        break
      }
      case 'tickets': {
        void navigate({ to: target.to })
        break
      }
      case 'guild': {
        void navigate({ to: '/serveurs/$guildId', params: { guildId: target.guildId } })
        break
      }
      case 'member': {
        void navigate({
          to: '/serveurs/$guildId/membres/$userId',
          params: { guildId: target.guildId, userId: target.memberId },
        })
        break
      }
      case 'contact': {
        void navigate({
          to: '/serveurs/$guildId/contacts',
          params: { guildId: target.guildId },
          search: { contact: target.contactId },
        })
        break
      }
      case 'item': {
        void navigate({
          to: '/serveurs/$guildId/coffres',
          params: { guildId: target.guildId },
          search: { objet: target.item },
        })
        break
      }
      case 'property': {
        void navigate({
          to: '/serveurs/$guildId/coffres',
          params: { guildId: target.guildId },
          search: { propriete: target.propertyId },
        })
        break
      }
      case 'cotisation': {
        setCotisationOpen(true)
        break
      }
      case 'theme': {
        setChoice(resolved === 'dark' ? 'light' : 'dark')
        break
      }
    }
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const last = flat.length - 1
    const moves: Record<string, number> = {
      ArrowDown: current >= last ? 0 : current + 1,
      ArrowUp: current <= 0 ? last : current - 1,
      Home: 0,
      End: last,
    }
    const next = moves[event.key]
    if (next !== undefined && flat.length > 0) {
      event.preventDefault()
      setActive(next)
      document.querySelector(`[id="${optionId(next)}"]`)?.scrollIntoView({ block: 'nearest' })
    } else if (event.key === 'Enter') {
      event.preventDefault()
      const command = flat[current]
      if (command) run(command)
    }
  }

  const searchingMembers = Boolean(guild) && debounced.length >= 2 && members.isFetching
  // Rang de la première entrée de chaque groupe dans la liste complète (navigation au clavier).
  const starts = sections.map((_, at) => sections.slice(0, at).reduce((sum, item) => sum + item.commands.length, 0))

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? setOpen(true) : close())}>
      <DialogContent
        showCloseButton={false}
        className="top-0 flex h-dvh max-h-dvh max-w-none translate-y-0 flex-col gap-0 rounded-none p-0 sm:top-[12vh] sm:h-auto sm:max-h-[70vh] sm:max-w-xl sm:rounded-lg"
      >
        <DialogTitle className="sr-only">Rechercher</DialogTitle>
        <DialogDescription className="sr-only">
          Pages, membres, contacts, objets des coffres et actions. Flèches pour choisir, Entrée pour ouvrir, Échap pour
          fermer.
        </DialogDescription>
        <div className="flex items-center gap-2 border-b px-4">
          <Search className="size-4 shrink-0 text-ink-subtle" aria-hidden />
          <input
            role="combobox"
            aria-expanded={flat.length > 0}
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={flat.length > 0 ? optionId(current) : undefined}
            aria-label="Rechercher une page, un membre, un contact, un objet…"
            placeholder="Rechercher une page, un membre, un contact…"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value)
              setActive(0)
            }}
            onKeyDown={onKeyDown}
            autoFocus
            autoComplete="off"
            spellCheck={false}
            className="h-14 min-w-0 flex-1 bg-transparent text-base text-ink outline-none placeholder:text-ink-subtle sm:text-sm"
          />
          <button
            type="button"
            onClick={close}
            className="min-h-11 rounded-md px-2 text-xs text-ink-muted hover:text-ink focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
          >
            Échap
          </button>
        </div>
        <div id={listId} role="listbox" aria-label="Résultats" className="min-h-0 flex-1 overflow-y-auto p-2">
          {sections.map((section, sectionIndex) => (
            <div key={section.group} role="group" aria-labelledby={`${listId}-${section.group}`} className="mb-2">
              <p
                id={`${listId}-${section.group}`}
                className="px-2 py-1.5 text-xs font-medium tracking-wide text-ink-subtle uppercase"
              >
                {section.label}
              </p>
              {section.commands.map((command, commandIndex) => {
                const position = (starts[sectionIndex] ?? 0) + commandIndex
                const selected = position === current
                return (
                  <div
                    key={command.id}
                    id={optionId(position)}
                    role="option"
                    aria-selected={selected}
                    tabIndex={-1}
                    onPointerMove={() => setActive(position)}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => run(command)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') run(command)
                    }}
                    className={cn(
                      'flex min-h-11 cursor-pointer items-center gap-3 rounded-md px-3 text-sm text-ink',
                      selected && 'bg-brand-soft text-brand',
                    )}
                  >
                    <span className="min-w-0 flex-1 truncate">{command.label}</span>
                    {command.hint && <span className="shrink-0 text-xs text-ink-subtle">{command.hint}</span>}
                    {selected && <CornerDownLeft className="size-3.5 shrink-0" aria-hidden />}
                  </div>
                )
              })}
            </div>
          ))}
          {flat.length === 0 && (
            <p className="px-3 py-8 text-center text-sm text-ink-muted" role="status">
              {searchingMembers ? 'Recherche…' : 'Aucun résultat.'}
            </p>
          )}
          {flat.length > 0 && searchingMembers && (
            <p className="px-3 py-2 text-xs text-ink-subtle" role="status">
              Recherche des membres…
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
