import { createFileRoute } from '@tanstack/react-router'

import { compact, searchDay, searchOneOf, searchPage, searchString } from '@/lib/search'
import type { TransactionsSearch } from '@/pages/transactions/transactions-search'
import { TransactionsPage } from '@/pages/transactions/TransactionsPage'

export const Route = createFileRoute('/serveurs/$guildId/transactions')({
  validateSearch: (search: Record<string, unknown>): TransactionsSearch =>
    compact({
      groupe: searchString(search.groupe),
      statut: searchString(search.statut),
      type: searchOneOf(search.type, ['ACHAT', 'VENTE'] as const),
      du: searchDay(search.du),
      au: searchDay(search.au),
      page: searchPage(search.page),
    }),
  component: TransactionsPage,
})
