// Ancienne adresse : /stats/<serveur> → /serveurs/<serveur>.

import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/stats/$guildId/')({
  beforeLoad: ({ params }) => {
    throw redirect({ to: '/serveurs/$guildId', params: { guildId: params.guildId }, replace: true })
  },
})
