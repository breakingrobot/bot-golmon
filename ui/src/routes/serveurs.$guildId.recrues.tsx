import { createFileRoute } from '@tanstack/react-router'

import { compact, searchString } from '@/lib/search'
import type { RecruesSearch } from '@/pages/recrues/recrues-search'
import { RecruesPage } from '@/pages/recrues/RecruesPage'

export const Route = createFileRoute('/serveurs/$guildId/recrues')({
  validateSearch: (search: Record<string, unknown>): RecruesSearch =>
    compact({ recrue: searchString(search['recrue']) }),
  component: RecruesPage,
})
