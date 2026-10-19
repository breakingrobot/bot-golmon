import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf, searchPage, searchString } from '@/lib/search'
import { PERIODS } from '@/pages/cotisations/cotisations-search'
import type { CotisationsSearch } from '@/pages/cotisations/cotisations-search'
import { CotisationsPage } from '@/pages/cotisations/CotisationsPage'

export const Route = createFileRoute('/serveurs/$guildId/cotisations')({
  validateSearch: (search: Record<string, unknown>): CotisationsSearch =>
    compact({
      jours: searchOneOf(
        search['jours'],
        PERIODS.map((period) => period.value),
      ),
      membre: searchString(search['membre']),
      page: searchPage(search['page']),
    }),
  component: CotisationsPage,
})
