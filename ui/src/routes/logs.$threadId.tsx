// Log d'un ticket. Adresse conservée telle quelle : les messages Discord y renvoient.

import { createFileRoute } from '@tanstack/react-router'

import { compact } from '@/lib/search'
import { flag } from '@/pages/logs/log-search'
import type { LogSearch } from '@/pages/logs/log-search'
import { LogPage } from '@/pages/logs/LogPage'

export const Route = createFileRoute('/logs/$threadId')({
  validateSearch: (search: Record<string, unknown>): LogSearch =>
    compact({ simple: flag(search['simple']), verbose: flag(search['verbose']) }),
  component: LogPage,
})
