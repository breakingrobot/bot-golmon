// Rendu de test : client TanStack Query neuf et contexte d'un serveur (comme sous /serveurs/$guildId).

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import type { ReactNode } from 'react'

import type { GuildAccess, Me } from '@/api/types'
import { GuildContext } from '@/lib/guild-context'

export const TEST_GUILD: GuildAccess = {
  id: '1187000000000000042',
  name: 'Les Corbeaux du Port',
  iconUrl: null,
  isAdmin: true,
  inGroup: true,
  perms: ['site.stats', 'contact.voir', 'contact.ajouter', 'contact.modifier'],
}

export const TEST_ME: Me = {
  user: { id: '3120000000000000017', name: 'Léo Marchetti', avatarUrl: null },
  loginEnabled: true,
  loginUrl: '/login?next=/',
  tickets: false,
  guilds: [TEST_GUILD],
}

export const createTestQueryClient = () =>
  new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })

export const renderInGuild = (ui: ReactNode, client = createTestQueryClient()) =>
  render(
    <QueryClientProvider client={client}>
      <GuildContext value={{ guild: TEST_GUILD, me: TEST_ME, userId: TEST_ME.user?.id ?? '' }}>{ui}</GuildContext>
    </QueryClientProvider>,
  )

/** Réponse JSON pour un `fetch` simulé. */
export const jsonResponse = (body: unknown, status = 200) => Promise.resolve(Response.json(body, { status }))
