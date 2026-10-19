import { createFileRoute } from '@tanstack/react-router'

import { StatsPage } from '@/pages/stats/StatsPage'

export const Route = createFileRoute('/serveurs/$guildId/')({ component: StatsPage })
