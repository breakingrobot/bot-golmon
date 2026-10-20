import { createFileRoute } from '@tanstack/react-router'

import { compact, searchDay, searchOneOf, searchPage, searchString } from '@/lib/search'
import { PERIODS, STATES, TABS } from '@/pages/presences/presences-search'
import type { PresencesSearch } from '@/pages/presences/presences-search'
import { PresencesPage } from '@/pages/presences/PresencesPage'

export const Route = createFileRoute('/serveurs/$guildId/presences')({
  validateSearch: (search: Record<string, unknown>): PresencesSearch =>
    compact({
      onglet: searchOneOf(
        search['onglet'],
        TABS.map((tab) => tab.value),
      ),
      etat: searchOneOf(
        search['etat'],
        STATES.map((state) => state.value),
      ),
      du: searchDay(search['du']),
      au: searchDay(search['au']),
      page: searchPage(search['page']),
      presence: searchString(search['presence']),
      jours: searchOneOf(
        search['jours'],
        PERIODS.map((period) => period.value),
      ),
    }),
  component: PresencesPage,
})
