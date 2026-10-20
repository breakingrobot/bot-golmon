/**
 * Contrat de l'API (suite de `types.ts`, mêmes conventions) : accueil d'un serveur, mise à jour en
 * direct et santé du bot.
 */

import type { MemberRef, ObjectiveProgress } from './types'

/**
 * GET /api/guilds/:g/overview — tuiles de l'accueil (membre du serveur).
 * Chaque tuile est `null` sans le droit indiqué (ou module non utilisé).
 */
export type GuildOverview = {
  /** `coffre.voir` : objets sous leur seuil (les 3 plus en manque). */
  lowStock: {
    count: number
    items: {
      propertyId: string
      propertyName: string | null
      item: string
      label: string
      quantity: number
      min: number
    }[]
  } | null
  /** `cotisation.ajouter` : avancement de la personne ; `reached`/`tracked` avec `cotisation.stats`. */
  objective: { progress: ObjectiveProgress | null; reached: number | null; tracked: number | null } | null
  /** `presence.stats` : présence ouverte la plus proche. */
  presence: {
    next: {
      id: string
      title: string
      date: string
      myStatus: 'PRESENT' | 'ABSENT' | 'RETARD' | 'NON_REPONDU'
      responses: number
    } | null
  } | null
  /** `presence.stats` : absents aujourd'hui (8 noms au plus). */
  absences: { count: number; members: MemberRef[] } | null
  /** `recrue.suivi` : recrues de la semaine ; avis manquants de la personne avec `recrue.avis`. */
  recrues: { count: number; missingVotes: number | null } | null
  /** `transaction.utiliser` : transactions en cours et montant engagé. */
  transactions: { count: number; amount: number } | null
  /** Staff des tickets, serveur principal du module ticket. */
  tickets: { open: number } | null
  /** `site.journal` : dernière entrée. */
  journal: { last: { at: string; actor: MemberRef; label: string } | null } | null
}

/**
 * GET /api/guilds/:g/events — flux SSE (`text/event-stream`, membre du serveur) :
 * `event: change`, `data: <sujet>` ; `*` = changements perdus, tout recharger.
 * Jamais de données : l'interface recharge les requêtes du sujet.
 */
export type LiveTopic =
  | 'coffres'
  | 'cotisations'
  | 'transactions'
  | 'contacts'
  | 'presences'
  | 'absences'
  | 'recrues'
  | 'journal'
  | 'config'

/** GET /api/guilds/:g/health?jours — santé du bot (« Gérer le serveur »). */
export type GuildHealth = {
  version: string
  commit: string | null
  startedAt: string
  uptimeSeconds: number
  databaseBytes: number | null
  messageContent: boolean
  shards: { id: number; connected: boolean; latencyMs: number | null; seenAt: string }[]
  /** Jours sans mouvement avant l'alerte « webhook silencieux » (7 par défaut). */
  silentDays: number
  channels: {
    channelId: string
    name: string | null
    accessible: boolean
    webhooks: number
    lastMovementAt: string | null
    lastCatchUpAt: string | null
    lastCatchUpOk: boolean | null
    silent: boolean
  }[]
  /** Vue globale : « Gérer le serveur » sur tous les serveurs du bot (erreurs sans serveur comprises). */
  global: boolean
  errors: { at: string; level: 'WARN' | 'ERROR'; target: string; message: string }[]
}
