// Requêtes des pages Présences, Absences, Recrues, Annuaire et écritures des transactions (suite de
// queries.ts : mêmes fabriques d'options et même mutation commune).

import { keepPreviousData, queryOptions } from '@tanstack/react-query'

import { api } from './client'
import { g, useGuildMutation } from './queries'
import type {
  AbsencesResponse,
  DeclaredAbsence,
  EndedAbsence,
  NewAbsence,
  NewPresence,
  NewPresenceSchedule,
  NewTransaction,
  Page,
  PhoneEntry,
  PresenceDetail,
  PresenceSchedule,
  PresenceStats,
  PresenceSummary,
  RecrueHistoryWeek,
  RecrueVoteInput,
  RecruesResponse,
  Transaction,
  TransactionItemInput,
  TransactionStatus,
  VehicleEntry,
  VehicleInput,
  VehiclesResponse,
  Veto,
} from './types'

export type PresenceFilters = { etat?: string; du?: string; au?: string }

export const presencesQuery = (guildId: string, filters: PresenceFilters, page: number) =>
  queryOptions({
    queryKey: ['guild', guildId, 'presences', 'list', filters, page],
    queryFn: ({ signal }) =>
      api<Page<PresenceSummary>>('GET', `${g(guildId)}/presences`, { query: { ...filters, page }, signal }),
    placeholderData: keepPreviousData,
  })

export const presenceQuery = (guildId: string, id: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'presences', 'detail', id],
    queryFn: ({ signal }) =>
      api<PresenceDetail>('GET', `${g(guildId)}/presences/${encodeURIComponent(id)}`, { signal }),
  })

export const presenceStatsQuery = (guildId: string, jours: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'presences', 'stats', jours],
    queryFn: ({ signal }) => api<PresenceStats>('GET', `${g(guildId)}/presences/stats`, { query: { jours }, signal }),
    placeholderData: keepPreviousData,
  })

export const presenceSchedulesQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'presences', 'plannings'],
    queryFn: ({ signal }) => api<PresenceSchedule[]>('GET', `${g(guildId)}/presences/plannings`, { signal }),
  })

export const absencesQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'absences'],
    queryFn: ({ signal }) => api<AbsencesResponse>('GET', `${g(guildId)}/absences`, { signal }),
  })

export const recruesQuery = (guildId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'recrues', 'week'],
    queryFn: ({ signal }) => api<RecruesResponse>('GET', `${g(guildId)}/recrues`, { signal }),
  })

export const recrueHistoryQuery = (guildId: string, userId: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'recrues', 'history', userId],
    queryFn: ({ signal }) =>
      api<RecrueHistoryWeek[]>('GET', `${g(guildId)}/recrues/${encodeURIComponent(userId)}/history`, { signal }),
  })

export const vehiclesQuery = (guildId: string, q: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'annuaire', 'vehicules', q],
    queryFn: ({ signal }) => api<VehiclesResponse>('GET', `${g(guildId)}/vehicules`, { query: { q }, signal }),
    placeholderData: keepPreviousData,
  })

export const phonesQuery = (guildId: string, q: string) =>
  queryOptions({
    queryKey: ['guild', guildId, 'annuaire', 'numeros', q],
    queryFn: ({ signal }) => api<PhoneEntry[]>('GET', `${g(guildId)}/numeros`, { query: { q }, signal }),
    placeholderData: keepPreviousData,
  })

export const useCreatePresence = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: NewPresence) => api<PresenceSummary>('POST', `${g(guildId)}/presences`, { body }),
    (presence) => `Présence « ${presence.title} » publiée sur Discord.`,
    [['presences'], ['stats']],
  )

export const useClosePresence = (guildId: string) =>
  useGuildMutation(
    guildId,
    (id: string) => api<PresenceDetail>('POST', `${g(guildId)}/presences/${encodeURIComponent(id)}/close`),
    'Présence clôturée.',
    [['presences'], ['stats'], ['member']],
  )

export const useCreatePresenceSchedule = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: NewPresenceSchedule) => api<PresenceSchedule>('POST', `${g(guildId)}/presences/plannings`, { body }),
    (schedule) => `Présence planifiée : ${schedule.weekdayLabel} à ${schedule.time}.`,
    [['presences', 'plannings']],
  )

export const useDeletePresenceSchedule = (guildId: string) =>
  useGuildMutation(
    guildId,
    (id: string) => api<void>('DELETE', `${g(guildId)}/presences/plannings/${encodeURIComponent(id)}`),
    'Planning supprimé. Les présences déjà publiées restent en place.',
    [['presences', 'plannings']],
  )

export const useDeclareAbsence = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: NewAbsence) => api<DeclaredAbsence>('POST', `${g(guildId)}/absences`, { body }),
    ({ affected, roleWarning }) =>
      [
        'Absence enregistrée.',
        affected > 0 ? `${affected} présence(s) ouverte(s) passée(s) en absent.` : '',
        roleWarning ? `Rôle d'absence non appliqué : ${roleWarning}.` : '',
      ]
        .filter(Boolean)
        .join(' '),
    [['absences'], ['presences'], ['stats'], ['member']],
  )

export const useEndAbsence = (guildId: string) =>
  useGuildMutation(
    guildId,
    (id: string) => api<EndedAbsence>('POST', `${g(guildId)}/absences/${encodeURIComponent(id)}/end`),
    ({ cancelled, roleWarning }) =>
      roleWarning
        ? `Rôle non retiré : ${roleWarning}. Le bot réessaiera.`
        : cancelled
          ? 'Absence annulée.'
          : 'Absence terminée.',
    [['absences'], ['stats'], ['member']],
  )

export const useVoteRecrue = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ entryId, body }: { entryId: string; body: RecrueVoteInput }) =>
      api<RecrueVoteInput>('PUT', `${g(guildId)}/recrues/${encodeURIComponent(entryId)}/vote`, { body }),
    'Avis enregistré.',
    [['recrues'], ['stats']],
  )

export const useSetRecrueVeto = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ entryId, motif }: { entryId: string; motif: string }) =>
      api<Veto>('PUT', `${g(guildId)}/recrues/${encodeURIComponent(entryId)}/veto`, { body: { motif } }),
    'Véto posé.',
    [['recrues'], ['stats'], ['journal']],
  )

export const useClearRecrueVeto = (guildId: string) =>
  useGuildMutation(
    guildId,
    (entryId: string) => api<void>('DELETE', `${g(guildId)}/recrues/${encodeURIComponent(entryId)}/veto`),
    'Véto levé.',
    [['recrues'], ['stats'], ['journal']],
  )

export const useCreateVehicle = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: VehicleInput) => api<VehicleEntry>('POST', `${g(guildId)}/vehicules`, { body }),
    (vehicle) => `${vehicle.plate}${vehicle.model ? ` (${vehicle.model})` : ''} enregistré.`,
    [['annuaire'], ['member']],
  )

export const useDeleteVehicle = (guildId: string) =>
  useGuildMutation(
    guildId,
    (plate: string) => api<void>('DELETE', `${g(guildId)}/vehicules/${encodeURIComponent(plate)}`),
    (_result, plate) => `${plate} retiré du registre.`,
    [['annuaire'], ['member']],
  )

export const useSetPhone = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ userId, number }: { userId: string; number: string }) =>
      api<PhoneEntry>('PUT', `${g(guildId)}/numeros/${encodeURIComponent(userId)}`, { body: { number } }),
    (entry) => `Numéro ${entry.number} enregistré.`,
    [['annuaire'], ['member']],
  )

export const useDeletePhone = (guildId: string) =>
  useGuildMutation(
    guildId,
    (userId: string) => api<void>('DELETE', `${g(guildId)}/numeros/${encodeURIComponent(userId)}`),
    'Numéro supprimé.',
    [['annuaire'], ['member']],
  )

export const useCreateTransaction = (guildId: string) =>
  useGuildMutation(
    guildId,
    (body: NewTransaction) => api<Transaction>('POST', `${g(guildId)}/transactions`, { body }),
    (transaction) => `Transaction pour « ${transaction.groupe} » créée et publiée sur Discord.`,
    [['transactions'], ['journal']],
  )

export const useSetTransactionStatus = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ id, status }: { id: string; status: TransactionStatus }) =>
      api<Transaction>('PUT', `${g(guildId)}/transactions/${encodeURIComponent(id)}/status`, { body: { status } }),
    (transaction) => `Statut passé à « ${transaction.statusLabel} ».`,
    [['transactions'], ['journal']],
  )

export const useAddTransactionItem = (guildId: string) =>
  useGuildMutation(
    guildId,
    ({ id, item }: { id: string; item: TransactionItemInput }) =>
      api<Transaction>('POST', `${g(guildId)}/transactions/${encodeURIComponent(id)}/items`, { body: item }),
    (_result, { item }) => `Objet « ${item.nom} » ajouté.`,
    [['transactions'], ['journal']],
  )

export const useCancelTransaction = (guildId: string) =>
  useGuildMutation(
    guildId,
    (id: string) => api<Transaction>('POST', `${g(guildId)}/transactions/${encodeURIComponent(id)}/cancel`),
    'Transaction annulée.',
    [['transactions'], ['journal']],
  )
