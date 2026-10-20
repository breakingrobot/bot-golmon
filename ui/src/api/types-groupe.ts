// Contrat des pages Présences, Absences, Recrues, Annuaire et des écritures des transactions (suite de
// types.ts, mêmes conventions ; réexporté par types.ts).

import type { MemberRef, Role } from './types'

// ---------------------------------------------------------------------------
// Présences — lecture `presence.stats`, création et clôture `presence.creer`, plannings `presence.gerer`
// GET    /api/guilds/:g/presences?etat&du&au&page            → Page<PresenceSummary>
// GET    /api/guilds/:g/presences/:id                         → PresenceDetail
// GET    /api/guilds/:g/presences/stats?jours                 → PresenceStats (jours absent = 30, `tout` = tout)
// GET    /api/guilds/:g/presences/export.csv?jours            — assiduité par membre
// POST   /api/guilds/:g/presences  NewPresence                → PresenceSummary (publiée sur Discord, ping habituel)
// POST   /api/guilds/:g/presences/:id/close                   → PresenceDetail
// GET    /api/guilds/:g/presences/plannings                   → PresenceSchedule[]
// POST   /api/guilds/:g/presences/plannings  NewPresenceSchedule → PresenceSchedule
// DELETE /api/guilds/:g/presences/plannings/:id

/** Réponse donnée à une présence. */
export type PresenceAnswer = 'PRESENT' | 'ABSENT' | 'RETARD'

export type PresenceSummary = {
  id: string
  title: string
  description: string | null
  /** Heure de l'événement. */
  date: string
  /** `null` : ouverte. */
  closedAt: string | null
  createdBy: MemberRef
  present: number
  absent: number
  retard: number
  /** Membres actuels du rôle sans réponse (comme le message Discord). */
  noAnswer: number
}

export type PresenceRespondent = { member: MemberRef; retardMinutes: number | null; changed: boolean }

export type PresenceDetail = {
  presence: PresenceSummary
  present: PresenceRespondent[]
  absent: PresenceRespondent[]
  retard: PresenceRespondent[]
  noAnswer: MemberRef[]
  /** Changements d'avis, plus récents d'abord. */
  changes: { member: MemberRef; from: PresenceAnswer | null; to: PresenceAnswer; at: string }[]
  /** Effectif actuel du rôle des présences. */
  roster: number
}

/** Assiduité d'un membre, règle du rapport hebdomadaire (présents ÷ attendus, un retard n'est pas présent). */
export type MemberAttendance = {
  member: MemberRef
  expected: number
  present: number
  retard: number
  absent: number
  noAnswer: number
  rate: number | null
}

export type PresenceStats = {
  /** Présences clôturées de la période. */
  presences: number
  rate: number | null
  /** Seuil d'alerte du rapport hebdomadaire, en %. */
  threshold: number
  /** Du plus assidu au moins assidu. */
  members: MemberAttendance[]
  /** Semaines de la période (12 dernières pour « tout »), vides comprises. */
  weekly: { weekStart: string; rate: number | null; presences: number }[]
}

export type PresenceSchedule = {
  id: string
  /** 1 = lundi … 7 = dimanche. */
  weekday: number
  weekdayLabel: string
  /** `HH:MM`, heure de Paris. */
  time: string
  title: string
  description: string | null
  /** Publication, en heures avant l'événement. */
  leadHours: number
  nextEvent: string | null
  createdBy: MemberRef
}

export type NewPresence = {
  /** `YYYY-MM-DD` ; absent : aujourd'hui, ou demain si l'heure est passée. */
  date?: string
  /** `HH:MM`, heure de Paris. */
  time: string
  /** Absent : « Présence ». 100 caractères au plus. */
  title?: string
  /** 1000 caractères au plus. */
  description?: string
}

export type NewPresenceSchedule = {
  weekday: number
  time: string
  title?: string
  description?: string
  /** 1 à 144 (24 par défaut). */
  leadHours?: number
}

// ---------------------------------------------------------------------------
// Absences — membres du groupe (`inGroup`) ; pour un autre membre : `absence.autrui`
// GET  /api/guilds/:g/absences                        → AbsencesResponse
// POST /api/guilds/:g/absences  NewAbsence            → DeclaredAbsence (annonce, rôle, présences ouvertes)
// POST /api/guilds/:g/absences/:id/end                → EndedAbsence (en cours : terminée ; à venir : annulée)

export type AbsenceEntry = {
  id: string
  member: MemberRef
  motif: string
  /** Début (00:00, heure de Paris). */
  from: string
  /** Fin prévue (23:59 du dernier jour). */
  to: string
  /** Fin anticipée ou annulation, `null` : en cours ou à venir. */
  endedAt: string | null
  createdBy: MemberRef
}

export type AbsencesResponse = {
  current: AbsenceEntry[]
  upcoming: AbsenceEntry[]
  /** Terminées ou annulées depuis 30 jours. */
  recent: AbsenceEntry[]
}

export type NewAbsence = {
  /** Absent : soi-même. */
  memberId?: string
  /** 200 caractères au plus. */
  motif: string
  /** Premier jour `YYYY-MM-DD` ; absent : aujourd'hui. */
  from?: string
  /** Dernier jour inclus `YYYY-MM-DD`. */
  to: string
}

export type DeclaredAbsence = {
  absence: AbsenceEntry
  /** Présences ouvertes de la période passées en absent. */
  affected: number
  /** Rôle d'absence non posé : raison. */
  roleWarning: string | null
}

export type EndedAbsence = { cancelled: boolean; roleWarning: string | null }

// ---------------------------------------------------------------------------
// Recrues — actions strictes (rôles seulement) : page ouverte avec `recrue.suivi`, `recrue.avis` ou `recrue.veto`
// GET    /api/guilds/:g/recrues                      → RecruesResponse (`votes` détaillés : `recrue.suivi`)
// GET    /api/guilds/:g/recrues/:userId/history      → RecrueHistoryWeek[] (`recrue.suivi`)
// PUT    /api/guilds/:g/recrues/:entryId/vote  RecrueVoteInput → { choice, motif } (`recrue.avis`)
// PUT    /api/guilds/:g/recrues/:entryId/veto  { motif }       → Veto (`recrue.veto`)
// DELETE /api/guilds/:g/recrues/:entryId/veto                   (`recrue.veto`)

export type RecrueChoice = 'POUR' | 'CONTRE' | 'BLANC'

export type RecrueVote = { voter: MemberRef; choice: RecrueChoice; motif: string }

export type Veto = { by: MemberRef; motif: string }

export type Recrue = {
  /** Suivi de la semaine (avis et vétos). */
  entryId: string
  member: MemberRef
  /** Première semaine suivie comme recrue. */
  since: string
  /** Semaines d'avis, celle-ci comprise. */
  weeks: number
  pour: number
  contre: number
  blanc: number
  veto: Veto | null
  /** Avis de la personne connectée. */
  myVote: { choice: RecrueChoice; motif: string } | null
  /** `null` sans `recrue.suivi`. */
  votes: RecrueVote[] | null
}

export type RecruesResponse = {
  /** Rôles recrue et gradé configurés. */
  configured: boolean
  /** Lundi de la semaine (jour de Paris), `null` sans semaine ouverte. */
  weekStart: string | null
  recruits: Recrue[]
}

export type RecrueHistoryWeek = {
  weekStart: string
  pour: number
  contre: number
  blanc: number
  veto: Veto | null
  votes: RecrueVote[]
}

/** Motif obligatoire, 500 caractères au plus. */
export type RecrueVoteInput = { choice: RecrueChoice; motif: string }

// ---------------------------------------------------------------------------
// Annuaire — véhicules `vehicule.utiliser`, numéros : membres du groupe (`inGroup`)
// GET    /api/guilds/:g/vehicules?q            → VehiclesResponse
// POST   /api/guilds/:g/vehicules  VehicleInput → VehicleEntry (modèle demandé à l'API GLife s'il manque)
// DELETE /api/guilds/:g/vehicules/:plate
// GET    /api/guilds/:g/numeros?q              → PhoneEntry[]
// PUT    /api/guilds/:g/numeros/:userId  { number } → PhoneEntry (autre membre : `numero.ajouter_autrui`)
// DELETE /api/guilds/:g/numeros/:userId               (autre membre : `numero.supprimer_autrui`)

export type VehicleEntry = {
  /** Plaque normalisée (majuscules, sans espace ni tiret). */
  plate: string
  model: string | null
  /** Propriétaire : un membre ou un rôle. */
  ownerMember: MemberRef | null
  ownerRole: Role | null
}

export type VehiclesResponse = {
  items: VehicleEntry[]
  /** Rôles du serveur, pour choisir un rôle propriétaire. */
  roles: Role[]
}

export type VehicleInput = { plate: string; ownerId: string; ownerIsRole?: boolean; model?: string }

/** `number` : format affiché `555-1234`. */
export type PhoneEntry = { member: MemberRef; number: string }

// ---------------------------------------------------------------------------
// Écritures des transactions — `transaction.utiliser` (fiche publiée dans le salon du tableau de bord)
// POST /api/guilds/:g/transactions  NewTransaction           → Transaction
// PUT  /api/guilds/:g/transactions/:id/status  { status }    → Transaction (tout sauf ANNULEE)
// POST /api/guilds/:g/transactions/:id/items  TransactionItemInput → Transaction
// POST /api/guilds/:g/transactions/:id/cancel                → Transaction
// Une transaction clôturée n'est plus modifiable (422).

export type TransactionItemInput = { nom: string; quantite?: number; prixUnitaire?: number }

export type NewTransaction = {
  groupe: string
  type: 'ACHAT' | 'VENTE'
  prix: number
  /** 25 au plus. */
  items?: TransactionItemInput[]
  /** Note libre, 500 caractères au plus. */
  note?: string
}
