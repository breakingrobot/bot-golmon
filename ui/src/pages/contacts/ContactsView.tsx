import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { BookUser, ChevronRight, Phone, Plus, Search, StickyNote, X } from 'lucide-react'
import { useEffect, useState } from 'react'

import { contactsQuery } from '@/api/queries'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Card } from '@/atoms/Card'
import { Input } from '@/atoms/Input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/atoms/Select'
import { useDebounced } from '@/hooks/use-debounced'
import { CONTACT_CSV, KIND_LABELS, RELATION_LABELS, RELATION_PLURALS, RELATION_VARIANT } from '@/lib/contact'
import { formatNumber } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect, searchOneOf } from '@/lib/search'
import type { SearchPatch } from '@/lib/search'
import { cn } from '@/lib/utils'
import { CsvExportButton } from '@/molecules/CsvExportButton'
import { EmptyState } from '@/molecules/EmptyState'
import { Field } from '@/molecules/Field'
import { PageHeader } from '@/molecules/PageHeader'
import { QueryError } from '@/molecules/QueryError'
import { StatCard } from '@/molecules/StatCard'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { ContactFormDialog } from '@/organisms/ContactFormDialog'
import type { ContactsSearch } from '@/pages/contacts/contacts-search'
import { KINDS, RELATIONS } from '@/pages/contacts/contacts-search'
import { ContactSheet } from '@/pages/contacts/ContactSheet'

const route = getRouteApi('/serveurs/$guildId/contacts')

export const ContactsView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/contacts' })
  const [text, setText] = useState(search.q ?? '')
  const q = useDebounced(text.trim(), 300)
  const filters = compact({ relation: search.relation, type: search.type, q: search.q })
  const query = useQuery(contactsQuery(guild.id, filters))
  const canAdd = hasPerm(guild, 'contact.ajouter')
  const setSearch = (patch: SearchPatch<ContactsSearch>) =>
    void navigate({ search: (prev) => compact({ ...prev, ...patch }), replace: true })

  useEffect(() => {
    if ((search.q ?? '') !== q) {
      void navigate({ search: (prev) => compact({ ...prev, q: q || undefined }), replace: true })
    }
  }, [q, search.q, navigate])

  const filtered = Boolean(search.relation || search.type || search.q)

  return (
    <>
      <PageHeader
        title="Contacts"
        description={`${guild.name} · groupes, entreprises et personnes`}
        actions={
          <>
            <CsvExportButton
              filename="contacts"
              columns={CONTACT_CSV}
              rows={query.data?.items ?? []}
              disabled={!query.data?.items.length}
            />
            {canAdd && (
              <ContactFormDialog
                guildId={guild.id}
                onSaved={(contact) => setSearch({ contact: contact.id })}
                trigger={
                  <Button size="sm">
                    <Plus aria-hidden /> Nouveau contact
                  </Button>
                }
              />
            )}
          </>
        }
      />

      <dl className="grid grid-cols-3 gap-3">
        {RELATIONS.map((relation) => {
          const count =
            query.data?.counts[relation === 'ALLIE' ? 'allie' : relation === 'NEUTRE' ? 'neutre' : 'hostile']
          return (
            <StatCard
              key={relation}
              label={RELATION_PLURALS[relation]}
              value={count === undefined ? '…' : formatNumber(count)}
              tone={relation === 'ALLIE' ? 'staff' : relation === 'HOSTILE' ? 'danger' : 'default'}
            />
          )
        })}
      </dl>

      <div className="mt-4 grid gap-3 rounded-xl border bg-card p-4 shadow-xs sm:grid-cols-2 lg:grid-cols-[2fr_1fr_1fr_auto]">
        <Field label="Recherche">
          {(props) => (
            <div className="relative">
              <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-ink-subtle" aria-hidden />
              <Input
                {...props}
                type="search"
                value={text}
                onChange={(event) => setText(event.target.value)}
                placeholder="Nom ou description"
                className="pl-9"
              />
            </div>
          )}
        </Field>
        <Field label="Relation">
          {(props) => (
            <Select
              value={search.relation ?? ALL}
              onValueChange={(value) => setSearch({ relation: searchOneOf(fromSelect(value), RELATIONS) })}
            >
              <SelectTrigger {...props}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>Toutes</SelectItem>
                {RELATIONS.map((relation) => (
                  <SelectItem key={relation} value={relation}>
                    {RELATION_PLURALS[relation]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </Field>
        <Field label="Type">
          {(props) => (
            <Select
              value={search.type ?? ALL}
              onValueChange={(value) => setSearch({ type: searchOneOf(fromSelect(value), KINDS) })}
            >
              <SelectTrigger {...props}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>Tous</SelectItem>
                {KINDS.map((kind) => (
                  <SelectItem key={kind} value={kind}>
                    {KIND_LABELS[kind]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </Field>
        <div className="flex items-end">
          {filtered && (
            <Button
              variant="ghost"
              onClick={() => {
                setText('')
                void navigate({ search: {}, replace: true })
              }}
            >
              <X aria-hidden /> Réinitialiser
            </Button>
          )}
        </div>
      </div>

      <section className="mt-6" aria-label="Répertoire">
        {query.isPending ? (
          <TableSkeleton />
        ) : query.isError ? (
          <QueryError error={query.error} retry={() => void query.refetch()} />
        ) : query.data.items.length === 0 ? (
          <EmptyState title="Aucun contact" icon={<BookUser />}>
            {filtered ? 'Aucun contact ne correspond aux filtres.' : 'Le répertoire est vide.'}
          </EmptyState>
        ) : (
          <Card className={cn('gap-0 py-0', query.isPlaceholderData && 'opacity-60')}>
            <ul className="divide-y">
              {query.data.items.map((contact) => (
                <li key={contact.id}>
                  <button
                    type="button"
                    onClick={() => setSearch({ contact: contact.id })}
                    className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 px-5 py-3 text-left hover:bg-muted/60 focus-visible:bg-muted focus-visible:outline-none"
                    aria-haspopup="dialog"
                  >
                    <span className="min-w-0 font-medium text-ink">{contact.name}</span>
                    <Badge variant={RELATION_VARIANT[contact.relation]}>{RELATION_LABELS[contact.relation]}</Badge>
                    <span className="text-sm text-ink-muted">{KIND_LABELS[contact.kind]}</span>
                    {contact.phone && (
                      <span className="inline-flex items-center gap-1 font-mono text-sm text-ink-muted">
                        <Phone className="size-3.5" aria-hidden />
                        {contact.phone}
                      </span>
                    )}
                    <span className="ml-auto flex items-center gap-3 text-xs text-ink-subtle">
                      {contact.notesCount !== null && (
                        <span className="inline-flex items-center gap-1">
                          <StickyNote className="size-3.5" aria-hidden />
                          {contact.notesCount} note{contact.notesCount > 1 ? 's' : ''}
                        </span>
                      )}
                      <ChevronRight className="size-4" aria-hidden />
                    </span>
                    {contact.description && (
                      <span className="w-full truncate text-sm text-ink-muted">{contact.description}</span>
                    )}
                  </button>
                </li>
              ))}
            </ul>
          </Card>
        )}
      </section>

      <ContactSheet guildId={guild.id} contactId={search.contact} onClose={() => setSearch({ contact: undefined })} />
    </>
  )
}
