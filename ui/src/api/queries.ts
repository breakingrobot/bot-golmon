// Requêtes TanStack Query : une fabrique d'options par route de l'API (voir types.ts).

import { keepPreviousData, queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { formatNumber } from '@/lib/format'

import { api, errorMessage } from './client'
import type {
  CoffreAlerts,
  CoffreInventory,
  CoffreInventoryResult,
  CoffreMeta,
  CoffrePlayer,
  CoffreSeries,
  CoffreStock,
  CoffreMovement,
  ConfigStatus,
  Contact,
  ContactDetail,
  ContactInput,
  ContactNote,
  ContactsResponse,
  CotisationEntry,
  CotisationsResponse,
  GuildStats,
  JournalResponse,
  Me,
  MemberProfile,
  MemberSearch,
  NewCotisation,
  Objective,
  Page,
  PermissionsResponse,
  PermissionsUpdate,
  TicketLog,
  TicketStats,
  TicketsResponse,
  TransactionsResponse,
} from './types'

export const g = (guildId: string) => `/guilds/${encodeURIComponent(guildId)}`

export const meQuery = queryOptions({
  queryKey: ['me'],
  queryFn: ({ signal }) => api<Me>('GET', '/me', { signal }),
  staleTime: 60_000,
})

export const statsQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'stats'],
    queryFn: ({ signal }) => api<GuildStats>('GET', `${g(guildId)}/stats`, { signal }),
  })

export const memberSearchQuery = (guildId: string, q: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'members', q],
    queryFn: ({ signal }) => api<MemberSearch>('GET', `${g(guildId)}/members`, { query: { q }, signal }),
    placeholderData: keepPreviousData,
    staleTime: 60_000,
  })

export const memberProfileQuery = (guildId: string, userId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'member', userId],
    queryFn: ({ signal }) =>
      api<MemberProfile>('GET', `${g(guildId)}/members/${encodeURIComponent(userId)}`, { signal }),
  })

export type TransactionFilters = { groupe?: string; statut?: string; type?: string; du?: string; au?: string }

export const transactionsQuery = (guildId: string, filters: TransactionFilters, page: number) =>
  queryOptions({
    queryKey: ['guild', guildId, 'transactions', filters, page],
    queryFn: ({ signal }) =>
      api<TransactionsResponse>('GET', `${g(guildId)}/transactions`, { query: { ...filters, page }, signal }),
    placeholderData: keepPreviousData,
  })

export type CotisationFilters = { jours: string; membre?: string }

export const cotisationsQuery = (guildId: string, filters: CotisationFilters, page: number) =>
  queryOptions({
    queryKey: ['guild', guildId, 'cotisations', filters, page],
    queryFn: ({ signal }) =>
      api<CotisationsResponse>('GET', `${g(guildId)}/cotisations`, { query: { ...filters, page }, signal }),
    placeholderData: keepPreviousData,
  })

export const objectiveQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'objective'],
    queryFn: ({ signal }) => api<Objective | null>('GET', `${g(guildId)}/cotisations/objective`, { signal }),
  })

export type CoffreFilters = { propriete?: string; objet?: string; joueur?: string; type?: string; jours: string }

export const coffreMetaQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'coffres', 'meta'],
    queryFn: ({ signal }) => api<CoffreMeta>('GET', `${g(guildId)}/coffres/meta`, { signal }),
  })

export const coffreStockQuery = (guildId: string, propriete: string | undefined) =>
  queryOptions({
    queryKey: ['guild', guildId, 'coffres', 'stock', propriete ?? ''],
    queryFn: ({ signal }) => api<CoffreStock>('GET', `${g(guildId)}/coffres/stock`, { query: { propriete }, signal }),
    placeholderData: keepPreviousData,
  })

export const coffreMovementsQuery = (guildId: string, filters: CoffreFilters, page: number) =>
  queryOptions({
    queryKey: ['guild', guildId, 'coffres', 'movements', filters, page],
    queryFn: ({ signal }) =>
      api<Page<CoffreMovement>>('GET', `${g(guildId)}/coffres/movements`, { query: { ...filters, page }, signal }),
    placeholderData: keepPreviousData,
  })

export const coffrePlayersQuery = (guildId: string, filters: CoffreFilters) =>
  queryOptions({
    queryKey: ['guild', guildId, 'coffres', 'players', filters],
    queryFn: ({ signal }) => api<CoffrePlayer[]>('GET', `${g(guildId)}/coffres/players`, { query: filters, signal }),
    placeholderData: keepPreviousData,
  })

export const coffreSeriesQuery = (guildId: string, objet: string, propriete: string | undefined, jours: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'coffres', 'series', objet, propriete ?? '', jours],
    queryFn: ({ signal }) =>
      api<CoffreSeries>('GET', `${g(guildId)}/coffres/series`, { query: { objet, propriete, jours }, signal }),
    placeholderData: keepPreviousData,
  })

export const coffreAlertsQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'coffres', 'alerts'],
    queryFn: ({ signal }) => api<CoffreAlerts>('GET', `${g(guildId)}/coffres/alerts`, { signal }),
  })

export type ContactFilters = { relation?: string; type?: string; q?: string }

export const contactsQuery = (guildId: string, filters: ContactFilters) =>
  queryOptions({
    queryKey: ['guild', guildId, 'contacts', 'list', filters],
    queryFn: ({ signal }) => api<ContactsResponse>('GET', `${g(guildId)}/contacts`, { query: filters, signal }),
    placeholderData: keepPreviousData,
  })

export const contactQuery = (guildId: string, id: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'contacts', 'detail', id],
    queryFn: ({ signal }) => api<ContactDetail>('GET', `${g(guildId)}/contacts/${encodeURIComponent(id)}`, { signal }),
  })

export const journalQuery = (guildId: string, action: string | undefined, page: number) =>
  queryOptions({
    queryKey: ['guild', guildId, 'journal', action ?? '', page],
    queryFn: ({ signal }) => api<JournalResponse>('GET', `${g(guildId)}/journal`, { query: { action, page }, signal }),
    placeholderData: keepPreviousData,
  })

export const permissionsQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'permissions'],
    queryFn: ({ signal }) => api<PermissionsResponse>('GET', `${g(guildId)}/permissions`, { signal }),
  })

export const configQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'config'],
    queryFn: ({ signal }) => api<ConfigStatus>('GET', `${g(guildId)}/config`, { signal }),
  })

export const ticketsQuery = (query: { q?: string; status?: string }, page: number) =>
  queryOptions({
    queryKey: ['tickets', query, page],
    queryFn: ({ signal }) => api<TicketsResponse>('GET', '/tickets', { query: { ...query, page }, signal }),
    placeholderData: keepPreviousData,
  })

export const ticketStaffQuery = queryOptions({
  queryKey: ['tickets', 'staff'],
  queryFn: ({ signal }) => api<TicketStats>('GET', '/tickets/staff', { signal }),
})

export const ticketLogQuery = (threadId: string) =>
  queryOptions({
    queryKey: ['log', threadId],
    queryFn: ({ signal }) => api<TicketLog>('GET', `/logs/${encodeURIComponent(threadId)}`, { signal }),
  })

// ---------------------------------------------------------------------------
// Écritures

/** Mutation commune : message de succès en toast, erreurs du serveur en toast, invalidation du serveur. */
export const useGuildMutation = <TVars, TResult>(
  guildId: string,
  run: (vars: TVars) => Promise<TResult>,
  success: string | ((result: TResult, vars: TVars) => string),
  invalidate: string[][],
) => {
  const client = useQueryClient()
  return useMutation({
    mutationFn: run,
    onSuccess: async (result, vars) => {
      toast.success(typeof success === 'string' ? success : success(result, vars))
      await Promise.all(invalidate.map((key) => client.invalidateQueries({ queryKey: ['guild', guildId, ...key] })))
    },
    onError: (error) => {
      toast.error(errorMessage(error))
    },
  })
}

export const useAddCotisation = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: NewCotisation) => api<CotisationEntry>('POST', `${g(guildId)}/cotisations`, { body }),
    'Cotisation enregistrée.',
    [['cotisations'], ['member']],
  )

export const useDeleteCotisation = (guildId: string) =>
  useGuildMutation(
    guildId,
    (id: string) => api<void>('DELETE', `${g(guildId)}/cotisations/${encodeURIComponent(id)}`),
    'Cotisation supprimée.',
    [['cotisations'], ['member']],
  )

export const useSaveObjective = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: Objective) => api<Objective>('PUT', `${g(guildId)}/cotisations/objective`, { body }),
    'Objectif enregistré.',
    [['objective'], ['cotisations'], ['config']],
  )

export const useRenameProperty = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ propertyId, name }: { propertyId: string; name: string }) =>
      api<unknown>('PUT', `${g(guildId)}/coffres/properties/${encodeURIComponent(propertyId)}`, { body: { name } }),
    (_result, vars) => (vars.name.trim() ? 'Propriété renommée.' : 'Nom de la propriété retiré.'),
    [['coffres']],
  )

export const useLinkPlayer = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ gameId, memberId }: { gameId: string; memberId: string }) =>
      api<unknown>('PUT', `${g(guildId)}/coffres/players/${encodeURIComponent(gameId)}`, { body: { memberId } }),
    'Personnage relié.',
    [['coffres'], ['member']],
  )

export const useUnlinkPlayer = (guildId: string) =>
  useGuildMutation(
    guildId,
    (gameId: string) => api<void>('DELETE', `${g(guildId)}/coffres/players/${encodeURIComponent(gameId)}`),
    'Personnage délié.',
    [['coffres'], ['member']],
  )

/**
 * Inventaire d'une propriété : les écarts deviennent des corrections. Les erreurs ne partent pas en toast :
 * le dialogue les affiche (`mutation.error`) pour que la saisie puisse être corrigée sur place.
 */
export const useSetCoffreInventory = (guildId: string) => {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (body: CoffreInventory) =>
      api<CoffreInventoryResult>('PUT', `${g(guildId)}/coffres/inventory`, { body }),
    onSuccess: async ({ corrections }) => {
      if (corrections.length === 0) toast.success('Le stock connu était déjà juste : aucune correction.')
      else {
        toast.success(`${corrections.length} correction(s) enregistrée(s)`, {
          description: corrections
            .map(({ label, before, after }) => `${label} : ${formatNumber(before)} → ${formatNumber(after)}`)
            .join(' · '),
        })
      }
      // Stock, mouvements, évolution et propriétés connues (une nouvelle propriété apparaît).
      await client.invalidateQueries({ queryKey: ['guild', guildId, 'coffres'] })
    },
  })
}

export const useSaveCoffreAlerts = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: CoffreAlerts) => api<CoffreAlerts>('PUT', `${g(guildId)}/coffres/alerts`, { body }),
    'Alertes enregistrées.',
    [['coffres', 'alerts'], ['config']],
  )

export const useCreateContact = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: ContactInput) => api<Contact>('POST', `${g(guildId)}/contacts`, { body }),
    (contact) => `Contact « ${contact.name} » ajouté.`,
    [['contacts']],
  )

export const useUpdateContact = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ id, body }: { id: string; body: Partial<ContactInput> }) =>
      api<Contact>('PATCH', `${g(guildId)}/contacts/${encodeURIComponent(id)}`, { body }),
    'Contact modifié.',
    [['contacts']],
  )

export const useDeleteContact = (guildId: string) =>
  useGuildMutation(
    guildId,
    (id: string) => api<void>('DELETE', `${g(guildId)}/contacts/${encodeURIComponent(id)}`),
    'Contact supprimé.',
    [['contacts']],
  )

export const useAddContactNote = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ id, body }: { id: string; body: string }) =>
      api<ContactNote>('POST', `${g(guildId)}/contacts/${encodeURIComponent(id)}/notes`, { body: { body } }),
    'Note ajoutée.',
    [['contacts']],
  )

export const useSavePermissions = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: PermissionsUpdate) => api<PermissionsResponse>('PUT', `${g(guildId)}/permissions`, { body }),
    'Permissions enregistrées.',
    [['permissions'], ['config']],
  )

export const useInvalidateMe = () => {
  const client = useQueryClient()
  return () => client.invalidateQueries({ queryKey: meQuery.queryKey })
}

export type { Query } from './client'
