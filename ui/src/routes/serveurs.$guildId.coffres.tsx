import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf, searchPage, searchString } from '@/lib/search'
import { MOVEMENT_TYPES, PERIODS } from '@/pages/coffres/coffres-search'
import type { CoffresSearch } from '@/pages/coffres/coffres-search'
import { CoffresPage } from '@/pages/coffres/CoffresPage'

export const Route = createFileRoute('/serveurs/$guildId/coffres')({
  validateSearch: (search: Record<string, unknown>): CoffresSearch =>
    compact({
      propriete: searchString(search['propriete']),
      objet: searchString(search['objet']),
      joueur: searchString(search['joueur']),
      type: searchOneOf(search['type'], MOVEMENT_TYPES),
      jours: searchOneOf(
        search['jours'],
        PERIODS.map((period) => period.value),
      ),
      page: searchPage(search['page']),
    }),
  component: CoffresPage,
})
