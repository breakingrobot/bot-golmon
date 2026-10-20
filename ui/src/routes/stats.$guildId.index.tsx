// Ancienne adresse : /stats/<serveur> → statistiques du serveur (/serveurs/<serveur>/stats).

import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/stats/$guildId/')({
  beforeLoad: ({ params, location }) => {
    throw redirect({
      to: '/serveurs/$guildId/stats',
      params: { guildId: params.guildId },
      search: location.search as Record<string, never>,
      replace: true,
    })
  },
})
