// Mise à jour en direct : sujet reçu du flux SSE du serveur → requêtes TanStack Query à recharger.
//
// Les clés des requêtes d'un serveur commencent toutes par `['guild', guildId, <domaine>, …]`
// (src/api/queries.ts) : un sujet invalide les domaines qu'il touche, quels que soient les filtres
// ou la page. Une nouvelle page d'un serveur range ses requêtes sous son domaine (`presences`,
// `absences`, `recrues`…) et l'ajoute ici au sujet qui la concerne.

import type { QueryKey } from '@tanstack/react-query'

import type { LiveTopic } from '@/api/types-site'

/** `*` : des changements ont été perdus (connexion lente ou reprise), tout recharger. */
export type LiveMessage = LiveTopic | '*'

export const LIVE_TOPICS: readonly LiveTopic[] = [
  'coffres',
  'cotisations',
  'transactions',
  'contacts',
  'presences',
  'absences',
  'recrues',
  'journal',
  'config',
]

/** Domaines (`['guild', id, …domaine]`) rechargés pour chaque sujet. */
export const TOPIC_DOMAINS: Record<LiveTopic, readonly (readonly string[])[]> = {
  coffres: [['coffres'], ['member'], ['overview'], ['health']],
  cotisations: [['cotisations'], ['objective'], ['member'], ['overview']],
  transactions: [['transactions'], ['stats'], ['overview']],
  contacts: [['contacts'], ['annuaire']],
  presences: [['presences'], ['stats'], ['member'], ['overview']],
  // Une absence exempte de l'objectif de cotisation de la semaine.
  absences: [['absences'], ['presences'], ['stats'], ['member'], ['overview'], ['cotisations']],
  recrues: [['recrues'], ['stats'], ['member'], ['overview']],
  journal: [['journal'], ['overview']],
  config: [['config'], ['permissions'], ['objective'], ['coffres', 'alerts'], ['health'], ['overview']],
}

export const isLiveMessage = (value: string): value is LiveMessage =>
  value === '*' || LIVE_TOPICS.some((topic) => topic === value)

/** Clés à invalider pour un message du flux. */
export const queryKeysFor = (guildId: string, message: LiveMessage): QueryKey[] => {
  // Tout le serveur, et la session : les droits ont pu changer pendant la coupure.
  if (message === '*') return [['guild', guildId], ['me']]
  const keys: QueryKey[] = TOPIC_DOMAINS[message].map((domain) => ['guild', guildId, ...domain])
  // Permissions modifiées : les pages ouvertes au compte aussi.
  if (message === 'config') keys.push(['me'])
  return keys
}

/** Attente avant la reconnexion n° `attempt` (0 = première) : exponentielle, plafonnée à 30 s, avec gigue. */
export const reconnectDelay = (attempt: number, random: () => number = Math.random): number =>
  Math.round(Math.min(30_000, 1000 * 2 ** attempt) * (0.5 + random() / 2))
