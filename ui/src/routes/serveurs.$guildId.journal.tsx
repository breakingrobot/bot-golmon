import { createFileRoute } from '@tanstack/react-router'

import { compact, searchPage, searchString } from '@/lib/search'
import type { JournalSearch } from '@/pages/journal/journal-search'
import { JournalPage } from '@/pages/journal/JournalPage'

export const Route = createFileRoute('/serveurs/$guildId/journal')({
  validateSearch: (search: Record<string, unknown>): JournalSearch =>
    compact({ action: searchString(search.action), page: searchPage(search.page) }),
  component: JournalPage,
})
