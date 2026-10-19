// Ancienne adresse : /cotisations → cotisations du premier serveur où la page est ouverte.

import { createFileRoute, redirect } from '@tanstack/react-router'

import { meQuery } from '@/api/queries'
import { hasPerm } from '@/lib/guild'

export const Route = createFileRoute('/cotisations')({
  beforeLoad: async ({ context, location }) => {
    const me = await context.queryClient.ensureQueryData(meQuery)
    const guild = me.guilds.find((item) => hasPerm(item, 'cotisation.stats'))
    if (!guild) throw redirect({ to: '/', replace: true })
    throw redirect({
      to: '/serveurs/$guildId/cotisations',
      params: { guildId: guild.id },
      search: location.search as Record<string, never>,
      replace: true,
    })
  },
})
