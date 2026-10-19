import { createContext, useContext } from 'react'

import type { GuildAccess, Me } from '@/api/types'

export type GuildContextValue = { guild: GuildAccess; me: Me; userId: string }

export const GuildContext = createContext<GuildContextValue | null>(null)

/** Serveur courant et compte connecté (fournis par la mise en page /serveurs/$guildId). */
export function useGuild(): GuildContextValue {
  const value = useContext(GuildContext)
  if (!value) throw new Error('useGuild doit être utilisé sous /serveurs/$guildId')
  return value
}
