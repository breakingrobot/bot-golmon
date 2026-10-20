// Requêtes des outils transverses du site (suite de `queries.ts`) : accueil, santé, suppressions
// avec « Annuler », exports CSV côté client.

import { keepPreviousData, queryOptions, useQueryClient } from '@tanstack/react-query'
import type { QueryClient, QueryKey } from '@tanstack/react-query'

import { useUndoableAction } from '@/hooks/use-undoable-action'

import { api } from './client'
import type {
  CoffreMeta,
  CoffrePlayer,
  CotisationEntry,
  CotisationsResponse,
  JournalResponse,
  MemberProfile,
  Page,
  TicketsResponse,
} from './types'
import type { GuildHealth, GuildOverview } from './types-site'

const g = (guildId: string) => `/guilds/${encodeURIComponent(guildId)}`

export const overviewQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'overview'],
    queryFn: ({ signal }) => api<GuildOverview>('GET', `${g(guildId)}/overview`, { signal }),
  })

export const healthQuery = (guildId: string, jours: string | undefined) =>
  queryOptions({
    queryKey: ['guild', guildId, 'health', jours ?? ''],
    queryFn: ({ signal }) => api<GuildHealth>('GET', `${g(guildId)}/health`, { query: { jours }, signal }),
    placeholderData: keepPreviousData,
  })

/** Modifie toutes les requêtes d'un domaine (tous filtres et pages) ; renvoie la remise en état. */
const patchQueries = <T>(client: QueryClient, queryKey: QueryKey, update: (data: T) => T): (() => void) => {
  void client.cancelQueries({ queryKey })
  const snapshot = client.getQueriesData<T>({ queryKey })
  client.setQueriesData<T>({ queryKey }, (data) => (data === undefined ? data : update(data)))
  return () => {
    for (const [key, data] of snapshot) client.setQueryData(key, data)
  }
}

const invalidateGuild = (client: QueryClient, guildId: string, domains: string[][]) =>
  Promise.all(domains.map((domain) => client.invalidateQueries({ queryKey: ['guild', guildId, ...domain] })))

const withoutEntry = (items: CotisationEntry[], id: string) => items.filter((item) => item.id !== id)

/** Supprime une cotisation avec « Annuler » (6 s) : liste et fiches mises à jour tout de suite. */
export const useUndoableDeleteCotisation = (guildId: string) => {
  const client = useQueryClient()
  const run = useUndoableAction()
  return (entry: CotisationEntry) =>
    run({
      key: `cotisation:${entry.id}`,
      message: 'Cotisation supprimée',
      hide: () => {
        const restoreList = patchQueries<CotisationsResponse>(client, ['guild', guildId, 'cotisations'], (data) =>
          data.entries.items.some((item) => item.id === entry.id)
            ? {
                ...data,
                entries: {
                  ...data.entries,
                  items: withoutEntry(data.entries.items, entry.id),
                  total: data.entries.total - 1,
                },
              }
            : data,
        )
        const restoreProfiles = patchQueries<MemberProfile>(client, ['guild', guildId, 'member'], (data) =>
          data.cotisations
            ? { ...data, cotisations: { ...data.cotisations, recent: withoutEntry(data.cotisations.recent, entry.id) } }
            : data,
        )
        return () => {
          restoreList()
          restoreProfiles()
        }
      },
      commit: ({ keepalive }) =>
        api<void>('DELETE', `${g(guildId)}/cotisations/${encodeURIComponent(entry.id)}`, { keepalive }),
      onCommitted: () => void invalidateGuild(client, guildId, [['cotisations'], ['member'], ['overview']]),
    })
}

/** Délie un personnage avec « Annuler » (6 s). */
export const useUndoableUnlinkPlayer = (guildId: string) => {
  const client = useQueryClient()
  const run = useUndoableAction()
  return ({ gameId, name }: { gameId: string; name: string }) =>
    run({
      key: `personnage:${gameId}`,
      message: `${name} délié`,
      hide: () => {
        const restoreMeta = patchQueries<CoffreMeta>(client, ['guild', guildId, 'coffres', 'meta'], (data) => ({
          ...data,
          links: data.links.filter((link) => link.gameId !== gameId),
        }))
        const restorePlayers = patchQueries<CoffrePlayer[]>(client, ['guild', guildId, 'coffres', 'players'], (data) =>
          data.map((player) => (player.gameId === gameId ? { ...player, member: null } : player)),
        )
        return () => {
          restoreMeta()
          restorePlayers()
        }
      },
      commit: ({ keepalive }) =>
        api<void>('DELETE', `${g(guildId)}/coffres/players/${encodeURIComponent(gameId)}`, { keepalive }),
      onCommitted: () => void invalidateGuild(client, guildId, [['coffres'], ['member']]),
    })
}

/** Toutes les pages d'une liste paginée, pour un export CSV (20 pages au plus). */
const allPages = async <T>(load: (page: number) => Promise<Page<T>>, maxPages = 20): Promise<T[]> => {
  const first = await load(1)
  const pages = Math.min(maxPages, Math.ceil(first.total / Math.max(1, first.pageSize)))
  const rest = await Promise.all(Array.from({ length: Math.max(0, pages - 1) }, (_, index) => load(index + 2)))
  return [first, ...rest].flatMap((page) => page.items)
}

export const fetchJournalForExport = (guildId: string, action: string | undefined) =>
  allPages(async (page) => {
    const response = await api<JournalResponse>('GET', `${g(guildId)}/journal`, { query: { action, page } })
    return response.list
  })

export const fetchTicketsForExport = (query: { q?: string; status?: string }) =>
  allPages((page) => api<TicketsResponse>('GET', '/tickets', { query: { ...query, page } }))
