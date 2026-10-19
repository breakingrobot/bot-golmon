import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf, searchPage, searchString } from '@/lib/search'
import { STATUSES } from '@/pages/tickets/tickets-search'
import type { TicketsSearch } from '@/pages/tickets/tickets-search'
import { TicketsPage } from '@/pages/tickets/TicketsPage'

export const Route = createFileRoute('/tickets/')({
  validateSearch: (search: Record<string, unknown>): TicketsSearch =>
    compact({
      q: searchString(search['q']),
      status: searchOneOf(search['status'], STATUSES),
      page: searchPage(search['page']),
    }),
  component: TicketsPage,
})
