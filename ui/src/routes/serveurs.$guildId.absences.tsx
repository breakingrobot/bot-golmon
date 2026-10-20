import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf } from '@/lib/search'
import { VIEWS } from '@/pages/absences/absences-search'
import type { AbsencesSearch } from '@/pages/absences/absences-search'
import { AbsencesPage } from '@/pages/absences/AbsencesPage'

export const Route = createFileRoute('/serveurs/$guildId/absences')({
  validateSearch: (search: Record<string, unknown>): AbsencesSearch =>
    compact({
      vue: searchOneOf(
        search['vue'],
        VIEWS.map((view) => view.value),
      ),
    }),
  component: AbsencesPage,
})
