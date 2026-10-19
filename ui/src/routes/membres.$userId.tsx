// Lien envoyé par le bot sur Discord : /membres/<id>?serveur=<id> → fiche du membre.

import { createFileRoute, redirect } from '@tanstack/react-router'

import { meQuery } from '@/api/queries'
import { compact, searchString } from '@/lib/search'

export const Route = createFileRoute('/membres/$userId')({
  validateSearch: (search: Record<string, unknown>): { serveur?: string } =>
    compact({ serveur: searchString(search['serveur']) }),
  beforeLoad: async ({ context, params, search }) => {
    const me = await context.queryClient.ensureQueryData(meQuery)
    const guildId = search.serveur ?? me.guilds[0]?.id
    if (!guildId) throw redirect({ to: '/' })
    throw redirect({ to: '/serveurs/$guildId/membres/$userId', params: { guildId, userId: params.userId } })
  },
})
