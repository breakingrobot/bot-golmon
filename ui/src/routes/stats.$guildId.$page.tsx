// Anciennes adresses : /stats/<serveur>/<page> → /serveurs/<serveur>/<page>, filtres conservés.

import { createFileRoute, notFound, redirect } from '@tanstack/react-router'

import type { GuildPagePath } from '@/lib/guild'

const PAGES: Record<string, GuildPagePath> = {
  transactions: '/serveurs/$guildId/transactions',
  cotisations: '/serveurs/$guildId/cotisations',
  coffres: '/serveurs/$guildId/coffres',
  contacts: '/serveurs/$guildId/contacts',
  journal: '/serveurs/$guildId/journal',
}

export const Route = createFileRoute('/stats/$guildId/$page')({
  beforeLoad: ({ params, location }) => {
    const to = PAGES[params.page]
    if (!to) throw notFound()
    // Les paramètres d'adresse (filtres) passent tels quels ; chaque page les valide.
    throw redirect({
      to,
      params: { guildId: params.guildId },
      search: location.search as Record<string, never>,
      replace: true,
    })
  },
})
