import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf, searchString } from '@/lib/search'
import { type ContactsSearch, KINDS, RELATIONS } from '@/pages/contacts/contacts-search'
import { ContactsPage } from '@/pages/contacts/ContactsPage'

export const Route = createFileRoute('/serveurs/$guildId/contacts')({
  validateSearch: (search: Record<string, unknown>): ContactsSearch =>
    compact({
      relation: searchOneOf(search.relation, RELATIONS),
      type: searchOneOf(search.type, KINDS),
      q: searchString(search.q),
      contact: searchString(search.contact),
    }),
  component: ContactsPage,
})
