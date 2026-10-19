import { createFileRoute } from '@tanstack/react-router'

import { compact, searchString } from '@/lib/search'
import { MembersPage } from '@/pages/members/MembersPage'

export const Route = createFileRoute('/serveurs/$guildId/membres/')({
  validateSearch: (search: Record<string, unknown>): { q?: string } => compact({ q: searchString(search['q']) }),
  component: MembersPage,
})
