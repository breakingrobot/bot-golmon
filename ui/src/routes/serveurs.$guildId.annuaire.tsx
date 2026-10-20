import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf, searchString } from '@/lib/search'
import { TABS } from '@/pages/annuaire/annuaire-search'
import type { AnnuaireSearch } from '@/pages/annuaire/annuaire-search'
import { AnnuairePage } from '@/pages/annuaire/AnnuairePage'

export const Route = createFileRoute('/serveurs/$guildId/annuaire')({
  validateSearch: (search: Record<string, unknown>): AnnuaireSearch =>
    compact({
      onglet: searchOneOf(
        search['onglet'],
        TABS.map((tab) => tab.value),
      ),
      q: searchString(search['q']),
    }),
  component: AnnuairePage,
})
