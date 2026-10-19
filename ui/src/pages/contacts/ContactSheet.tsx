import { useQuery } from '@tanstack/react-query'
import { Pencil, Phone } from 'lucide-react'

import { contactQuery } from '@/api/queries'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/atoms/Sheet'
import { Skeleton } from '@/atoms/Skeleton'
import { KIND_LABELS, RELATION_LABELS, RELATION_VARIANT } from '@/lib/contact'
import { formatDate, formatDateTime, formatRelative } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { MemberName } from '@/molecules/MemberName'
import { QueryError } from '@/molecules/QueryError'
import { ContactFormDialog } from '@/organisms/ContactFormDialog'
import { AddNote } from '@/pages/contacts/AddNote'
import { DeleteContact } from '@/pages/contacts/DeleteContact'

export function ContactSheet({
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
