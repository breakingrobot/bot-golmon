import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { BookUser, ChevronRight, Pencil, Phone, Plus, Search, StickyNote, Trash2, X } from 'lucide-react'
import { useEffect, useState, type FormEvent } from 'react'

import { contactQuery, contactsQuery, useAddContactNote, useDeleteContact } from '@/api/queries'
import type { Contact, ContactKind, Relation } from '@/api/types'
import {
  ContactFormDialog,
  KIND_LABELS,
  LIMITS,
  RELATION_LABELS,
  RELATION_PLURALS,
  RELATION_VARIANT,
} from '@/components/contact-form'
import { CharCount, Field } from '@/components/form'
import { MemberName } from '@/components/member'
import { EmptyState, Forbidden, PageHeader, QueryError, StatCard, TableSkeleton } from '@/components/page'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input, Textarea } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/misc'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { useDebounced } from '@/hooks/use-debounced'
import { formatDate, formatDateTime, formatNumber, formatRelative } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ALL, compact, fromSelect, searchOneOf, searchString, type SearchPatch } from '@/lib/search'
import { cn } from '@/lib/utils'

const RELATIONS = ['ALLIE', 'NEUTRE', 'HOSTILE'] as const
const KINDS = ['GROUPE', 'ENTREPRISE', 'PERSONNE'] as const

type Search = { relation?: Relation; type?: ContactKind; q?: string; contact?: string }

export const Route = createFileRoute('/serveurs/$guildId/contacts')({
  validateSearch: (search: Record<string, unknown>): Search =>
    compact({
      relation: searchOneOf(search.relation, RELATIONS),
      type: searchOneOf(search.type, KINDS),
      q: searchString(search.q),
      contact: searchString(search.contact),
    }),
  component: ContactsPage,
})

function ContactsPage() {
  const { guild } = useGuild()
  if (!hasPerm(guild, 'contact.voir')) return <Forbidden />
  return <Contacts />
}

function Contacts() {
  const { guild } = useGuild()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const [text, setText] = useState(search.q ?? '')
  const q = useDebounced(text.trim(), 300)
  const filters = compact({ relation: search.relation, type: search.type, q: search.q })
  const query = useQuery(contactsQuery(guild.id, filters))
  const canAdd = hasPerm(guild, 'contact.ajouter')
  const setSearch = (patch: SearchPatch<Search>) =>
    void navigate({ search: (prev) => compact({ ...prev, ...patch }), replace: true })

  useEffect(() => {
    if ((search.q ?? '') !== q)
      void navigate({ search: (prev) => compact({ ...prev, q: q || undefined }), replace: true })
  }, [q, search.q, navigate])

  const filtered = Boolean(search.relation || search.type || search.q)

  return (
    <>
      <PageHeader
        title="Contacts"
        description={`${guild.name} · groupes, entreprises et personnes`}
        actions={
          canAdd && (
            <ContactFormDialog
              guildId={guild.id}
              onSaved={(contact) => setSearch({ contact: contact.id })}
              trigger={
                <Button size="sm">
                  <Plus aria-hidden /> Nouveau contact
                </Button>
              }
            />
          )
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

function ContactSheet({
  guildId,
  contactId,
  onClose,
}: {
  guildId: string
  contactId: string | undefined
  onClose: () => void
}) {
  const { guild } = useGuild()
  const detail = useQuery({ ...contactQuery(guildId, contactId ?? ''), enabled: Boolean(contactId) })
  const canEdit = hasPerm(guild, 'contact.modifier')
  const canNotes = hasPerm(guild, 'contact.notes')
  const contact = detail.data?.contact

  return (
    <Sheet open={Boolean(contactId)} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="overflow-y-auto">
        <SheetHeader>
          <SheetTitle>{contact?.name ?? 'Contact'}</SheetTitle>
          <SheetDescription>
            {contact ? `${KIND_LABELS[contact.kind]} · ${RELATION_LABELS[contact.relation]}` : 'Chargement…'}
          </SheetDescription>
        </SheetHeader>
        <div className="grid gap-6 px-4 pb-8">
          {detail.isPending ? (
            <div className="space-y-3">
              <Skeleton className="h-6 w-40" />
              <Skeleton className="h-20" />
              <Skeleton className="h-32" />
            </div>
          ) : detail.isError ? (
            <QueryError error={detail.error} retry={() => void detail.refetch()} />
          ) : contact ? (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={RELATION_VARIANT[contact.relation]}>{RELATION_LABELS[contact.relation]}</Badge>
                <Badge variant="outline">{KIND_LABELS[contact.kind]}</Badge>
                {contact.phone && (
                  <span className="inline-flex items-center gap-1 font-mono text-sm text-ink">
                    <Phone className="size-3.5" aria-hidden />
                    {contact.phone}
                  </span>
                )}
              </div>
              <p className="text-sm whitespace-pre-line text-ink">
                {contact.description ?? <span className="text-ink-subtle">Pas de description.</span>}
              </p>
              <p className="flex flex-wrap items-center gap-1 text-xs text-ink-subtle">
                Ajouté par{' '}
                <MemberName member={contact.createdBy} guildId={guildId} avatar={false} className="text-xs" /> le{' '}
                {formatDate(contact.createdAt)} · mis à jour {formatRelative(contact.updatedAt)}
              </p>
              {canEdit && (
                <div className="flex flex-wrap gap-2">
                  <ContactFormDialog
                    guildId={guildId}
                    contact={contact}
                    trigger={
                      <Button variant="outline" size="sm">
                        <Pencil aria-hidden /> Modifier
                      </Button>
                    }
                  />
                  <DeleteContact contact={contact} onDeleted={onClose} />
                </div>
              )}
              {contact.notesCount !== null && (
                <section aria-labelledby="notes-titre">
                  <h3 id="notes-titre" className="text-xs font-medium tracking-wide text-ink-subtle uppercase">
                    Notes du staff
                  </h3>
                  {canNotes && <AddNote guildId={guildId} contactId={contact.id} />}
                  {detail.data.notes.length === 0 ? (
                    <p className="mt-3 text-sm text-ink-muted">Aucune note.</p>
                  ) : (
                    <ul className="mt-3 space-y-2">
                      {detail.data.notes.map((note) => (
                        <li key={note.id} className="rounded-lg border bg-muted/40 px-3 py-2">
                          <p className="flex flex-wrap items-center gap-1 text-xs text-ink-subtle">
                            <MemberName
                              member={note.author}
                              guildId={guildId}
                              avatar={false}
                              className="text-xs text-ink-muted"
                            />
                            · <time dateTime={note.createdAt}>{formatDateTime(note.createdAt)}</time>
                          </p>
                          <p className="mt-1 text-sm whitespace-pre-line text-ink">{note.body}</p>
                        </li>
                      ))}
                    </ul>
                  )}
                </section>
              )}
            </>
          ) : null}
        </div>
      </SheetContent>
    </Sheet>
  )
}

function AddNote({ guildId, contactId }: { guildId: string; contactId: string }) {
  const [body, setBody] = useState('')
  const [error, setError] = useState<string | null>(null)
  const mutation = useAddContactNote(guildId)
  const submit = (event: FormEvent) => {
    event.preventDefault()
    const text = body.trim()
    if (!text) return setError('La note est vide.')
    if (text.length > LIMITS.note) return setError(`${LIMITS.note} caractères au plus.`)
    setError(null)
    mutation.mutate({ id: contactId, body: text }, { onSuccess: () => setBody('') })
  }
  return (
    <form className="mt-3 grid gap-2" onSubmit={submit} noValidate>
      <Field label="Nouvelle note" error={error} hint={<CharCount value={body} max={LIMITS.note} />}>
        {(props) => (
          <Textarea
            {...props}
            value={body}
            onChange={(event) => setBody(event.target.value)}
            rows={3}
            placeholder="Visible par le staff seulement"
          />
        )}
      </Field>
      <div>
        <Button type="submit" size="sm" disabled={mutation.isPending}>
          <StickyNote aria-hidden /> Ajouter la note
        </Button>
      </div>
    </form>
  )
}

function DeleteContact({ contact, onDeleted }: { contact: Contact; onDeleted: () => void }) {
  const { guild } = useGuild()
  const mutation = useDeleteContact(guild.id)
  const [confirm, setConfirm] = useState('')
  return (
    <AlertDialog onOpenChange={() => setConfirm('')}>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size="sm" className="text-danger hover:text-danger">
          <Trash2 aria-hidden /> Supprimer
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Supprimer « {contact.name} » ?</AlertDialogTitle>
          <AlertDialogDescription>
            Le contact et ses notes disparaissent du répertoire, sur le site et sur Discord. Tapez le nom pour
            confirmer.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <Input value={confirm} onChange={(event) => setConfirm(event.target.value)} aria-label="Nom du contact" />
        <AlertDialogFooter>
          <AlertDialogCancel>Annuler</AlertDialogCancel>
          <AlertDialogAction
            disabled={confirm.trim() !== contact.name || mutation.isPending}
            onClick={(event) => {
              event.preventDefault()
              mutation.mutate(contact.id, { onSuccess: onDeleted })
            }}
          >
            Supprimer
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
