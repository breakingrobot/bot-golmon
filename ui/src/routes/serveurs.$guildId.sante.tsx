import { createFileRoute } from '@tanstack/react-router'

import { compact, searchOneOf } from '@/lib/search'
import { SILENT_PERIODS } from '@/pages/health/health-search'
import type { HealthSearch } from '@/pages/health/health-search'
import { HealthPage } from '@/pages/health/HealthPage'

export const Route = createFileRoute('/serveurs/$guildId/sante')({
  validateSearch: (search: Record<string, unknown>): HealthSearch =>
    compact({ jours: searchOneOf(search['jours'], SILENT_PERIODS) }),
  component: HealthPage,
})
