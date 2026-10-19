// Données de maquette : un groupe RP fictif, généré de façon déterministe (mêmes données à chaque chargement).

import type {
  CoffreAlerts,
  CoffreMovement,
  Contact,
  ContactNote,
  CotisationEntry,
  GuildAccess,
  JournalEntry,
  Level,
  LogEntry,
  MemberRef,
  Objective,
  Perm,
  PermissionsResponse,
  Role,
  TicketRow,
  TicketStatus,
  Transaction,
  TransactionStatus,
} from '@/api/types'

// ---------------------------------------------------------------------------
// Outils

function mulberry32(seed: number) {
  let state = seed
  return () => {
    state = (state + 0x6d2b79f5) | 0
    let t = Math.imul(state ^ (state >>> 15), 1 | state)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

export const random = mulberry32(20261019)

export function pick<T>(items: readonly T[]): T {
  const item = items[Math.floor(random() * items.length)]
  if (item === undefined) throw new Error('liste vide')
  return item
}

export function int(min: number, max: number): number {
  return Math.floor(min + random() * (max - min + 1))
}

const DAY = 86_400_000
export const NOW = Date.now()

/** Instant ISO il y a `days` jours (fraction possible), heure de soirée RP. */
export function daysAgo(days: number, hour = int(17, 23), minute = int(0, 59)): string {
  const date = new Date(NOW - Math.floor(days) * DAY)
  date.setUTCHours(hour - 2, minute, int(0, 59), 0)
  if (date.getTime() > NOW) date.setTime(NOW - int(10, 600) * 60_000)
  return date.toISOString()
}

/** Jour `YYYY-MM-DD` (heure de Paris) d'un instant. */
export function parisDay(iso: string | number): string {
  return new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Paris' }).format(new Date(iso))
}

/** Lundi (jour de Paris) de la semaine d'un jour `YYYY-MM-DD`. */
export function mondayOf(day: string): string {
  const date = new Date(`${day}T12:00:00Z`)
  const weekday = (date.getUTCDay() + 6) % 7
  return new Date(date.getTime() - weekday * DAY).toISOString().slice(0, 10)
}

export function addDays(day: string, days: number): string {
  return new Date(new Date(`${day}T12:00:00Z`).getTime() + days * DAY).toISOString().slice(0, 10)
}

let counter = 1
export function nextId(): string {
  counter += 1
  return `${1300000000000000000n + BigInt(counter * 7919)}`
}

// ---------------------------------------------------------------------------
// Membres et serveurs

const NAMES = [
  'Léo Marchetti',
  'Inès Benali',
  'Hugo Delcourt',
  'Sacha Morel',
  'Camille Roux',
  'Yanis Ferrand',
  'Manon Lefèvre',
  'Théo Garnier',
  'Nina Castel',
  'Bastien Royer',
  'Lucas Vidal',
  'Chloé Fabre',
  'Enzo Martel',
  'Jade Perrin',
  'Mathis Caron',
  'Lina Brunet',
] as const

export const MEMBERS: MemberRef[] = NAMES.map((name, index) => ({
  id: `${3120000000000000000n + BigInt(index * 104_729 + 17)}`,
  name,
  avatarUrl: null,
}))

export function member(index: number): MemberRef {
  const found = MEMBERS[index % MEMBERS.length]
  if (!found) throw new Error('membre absent')
  return found
}

export const ME = member(0)

export const GUILD_ID = '1187000000000000042'
export const GUILD_B_ID = '1187000000000000077'

export const ROLES: Role[] = [
  { id: '1188000000000000001', name: 'Fondateur', color: 0xc0392b },
  { id: '1188000000000000002', name: 'Bras droit', color: 0xe67e22 },
  { id: '1188000000000000003', name: 'Gradé', color: 0x8e44ad },
  { id: '1188000000000000004', name: 'Membre', color: 0x3498db },
  { id: '1188000000000000005', name: 'Recrue', color: 0x95a5a6 },
  { id: '1188000000000000006', name: 'Mécano', color: 0x16a085 },
  { id: '1188000000000000007', name: 'Bot', color: null },
]

export function role(name: string): Role {
  const found = ROLES.find((item) => item.name === name)
  if (!found) throw new Error(`rôle ${name}`)
  return found
}

/** Rôles de chaque membre (index dans MEMBERS). */
export function memberRoles(index: number): Role[] {
  if (index === 0) return [role('Fondateur'), role('Gradé'), role('Membre')]
  if (index === 1) return [role('Bras droit'), role('Gradé'), role('Membre')]
  if (index <= 4) return [role('Gradé'), role('Membre'), ...(index === 3 ? [role('Mécano')] : [])]
  if (index >= 14) return [role('Recrue')]
  return [role('Membre'), ...(index % 4 === 0 ? [role('Mécano')] : [])]
}

export const ALL_PERMS: Perm[] = [
  'presence.creer',
  'presence.stats',
  'presence.gerer',
  'presence.reset',
  'presence.blocklist',
  'absence.autrui',
  'transaction.utiliser',
  'transaction.supprimer',
  'transaction.historique',
  'vehicule.utiliser',
  'numero.ajouter_autrui',
  'numero.supprimer_autrui',
  'recrue.suivi',
  'recrue.avis',
  'recrue.veto',
  'recrue.reset',
  'cotisation.ajouter',
  'cotisation.gerer',
  'cotisation.stats',
  'coffre.voir',
  'coffre.gerer',
  'contact.voir',
  'contact.ajouter',
  'contact.modifier',
  'contact.notes',
  'membre.fiche',
  'site.stats',
  'site.journal',
]

export const GUILDS: GuildAccess[] = [
  {
    id: GUILD_ID,
    name: 'Les Corbeaux du Port',
    iconUrl: null,
    isAdmin: true,
    perms: ALL_PERMS,
  },
  {
    id: GUILD_B_ID,
    name: 'Brasserie du Vieux Port',
    iconUrl: null,
    isAdmin: false,
    perms: ['presence.creer', 'cotisation.ajouter', 'contact.voir', 'contact.ajouter', 'vehicule.utiliser'],
  },
]

// ---------------------------------------------------------------------------
// Permissions

type ActionDef = { key: Perm; module: string; label: string; defaultLevel: Level; strict: boolean }

export const ACTIONS: ActionDef[] = [
  {
    key: 'presence.creer',
    module: 'Présences',
    label: 'Créer, relancer et clôturer une présence',
    defaultLevel: 'tous',
    strict: false,
  },
  {
    key: 'presence.stats',
    module: 'Présences',
    label: "Statistiques et historique d'une présence",
    defaultLevel: 'tous',
    strict: false,
  },
  {
    key: 'presence.gerer',
    module: 'Présences',
    label: 'Forcer une réponse, annuler une présence, planning',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'presence.reset',
    module: 'Présences',
    label: 'Réinitialiser les statistiques de présence',
    defaultLevel: 'admin',
    strict: false,
  },
  {
    key: 'presence.blocklist',
    module: 'Présences',
    label: 'Ajouter ou retirer un membre de la blocklist',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'absence.autrui',
    module: 'Absences',
    label: "Absence d'un autre membre, panneau des absences",
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'transaction.utiliser',
    module: 'Transactions',
    label: 'Créer, compléter, changer le statut, annuler',
    defaultLevel: 'tous',
    strict: false,
  },
  {
    key: 'transaction.supprimer',
    module: 'Transactions',
    label: 'Supprimer définitivement une transaction',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'transaction.historique',
    module: 'Transactions',
    label: 'Historique et export sur le site',
    defaultLevel: 'admin',
    strict: false,
  },
  {
    key: 'vehicule.utiliser',
    module: 'Véhicules',
    label: 'Enregistrer, consulter, modifier, retirer un véhicule',
    defaultLevel: 'tous',
    strict: false,
  },
  {
    key: 'numero.ajouter_autrui',
    module: 'Numéros',
    label: "Enregistrer le numéro d'un autre membre",
    defaultLevel: 'admin',
    strict: false,
  },
  {
    key: 'numero.supprimer_autrui',
    module: 'Numéros',
    label: "Supprimer le numéro d'un autre membre",
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'recrue.suivi',
    module: 'Recrues',
    label: "Liste, détail et historique des avis, ajout et retrait, salon d'essai",
    defaultLevel: 'grade',
    strict: true,
  },
  {
    key: 'recrue.avis',
    module: 'Recrues',
    label: 'Donner un avis sur une recrue',
    defaultLevel: 'grade',
    strict: true,
  },
  { key: 'recrue.veto', module: 'Recrues', label: 'Poser ou lever un véto', defaultLevel: 'lead', strict: true },
  {
    key: 'recrue.reset',
    module: 'Recrues',
    label: "Forcer une nouvelle semaine d'avis",
    defaultLevel: 'admin',
    strict: false,
  },
  {
    key: 'cotisation.ajouter',
    module: 'Cotisations',
    label: 'Déclarer ses cotisations',
    defaultLevel: 'tous',
    strict: false,
  },
  {
    key: 'cotisation.gerer',
    module: 'Cotisations',
    label: 'Cotisations des autres : déclarer, consulter, supprimer',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'cotisation.stats',
    module: 'Cotisations',
    label: 'Statistiques (Discord et site)',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'coffre.voir',
    module: 'Coffres',
    label: 'Stock et historique (Discord et site)',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'coffre.gerer',
    module: 'Coffres',
    label: 'Relier un personnage, nommer une propriété',
    defaultLevel: 'grade',
    strict: false,
  },
  { key: 'contact.voir', module: 'Contacts', label: 'Consulter le répertoire', defaultLevel: 'tous', strict: false },
  { key: 'contact.ajouter', module: 'Contacts', label: 'Ajouter un contact', defaultLevel: 'membre', strict: false },
  {
    key: 'contact.modifier',
    module: 'Contacts',
    label: 'Modifier ou supprimer un contact',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'contact.notes',
    module: 'Contacts',
    label: 'Lire et écrire les notes du staff (Discord et site)',
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'membre.fiche',
    module: 'Membres',
    label: "Fiche d'un membre (présences, cotisations, coffres, recrue…)",
    defaultLevel: 'grade',
    strict: false,
  },
  {
    key: 'site.stats',
    module: 'Site',
    label: 'Statistiques du serveur sur le site',
    defaultLevel: 'admin',
    strict: false,
  },
  { key: 'site.journal', module: 'Site', label: 'Journal du staff sur le site', defaultLevel: 'admin', strict: false },
]

export const permissionLevels = new Map<Perm, Level>([
  ['coffre.voir', 'membre'],
  ['site.stats', 'grade'],
  ['contact.notes', 'lead'],
])

export const permissionRoles: { memberRoleId: string | null } = { memberRoleId: role('Membre').id }

export function permissionsResponse(): PermissionsResponse {
  return {
    roles: {
      member: ROLES.find((item) => item.id === permissionRoles.memberRoleId) ?? null,
      grade: role('Gradé'),
      lead: role('Bras droit'),
    },
    guildRoles: ROLES,
    actions: ACTIONS.map((action) => ({ ...action, level: permissionLevels.get(action.key) ?? action.defaultLevel })),
  }
}

// ---------------------------------------------------------------------------
// Transactions

const GROUPES = [
  'Famille Ortega',
  'Les Chacals',
  'Atelier Mécano Sud',
  'Club Le Velvet',
  'Groupe Kovač',
  'Marchand du port',
]
const OBJETS: [string, number][] = [
  ['Pistolet', 4500],
  ['Munitions 9 mm', 12],
  ['Kit de crochetage', 350],
  ['Gilet pare-balles', 1800],
  ['Pièces moteur', 600],
  ['Radio cryptée', 950],
  ['Pochon de beuh', 85],
  ['Bidon d’essence', 120],
]

export const STATUS_LABELS: Record<TransactionStatus, string> = {
  PREPARATION: 'Préparation',
  EN_ATTENTE_RDV: 'En attente RDV',
  AVANCE_ATTENTE_PAIEMENT: 'Avance (en attente de paiement)',
  LIVRE_PAYE: 'Livré et payé',
  ANNULEE: 'Annulée',
  CLOTUREE: 'Clôturée',
}

export const TRANSACTIONS: Transaction[] = Array.from({ length: 86 }, (_, index) => {
  const age = index < 4 ? index * 1.5 : Math.min(330, index * 3.8 + random() * 3)
  const status: TransactionStatus =
    index < 4
      ? pick(['PREPARATION', 'EN_ATTENTE_RDV', 'AVANCE_ATTENTE_PAIEMENT'] as const)
      : random() < 0.08
        ? 'ANNULEE'
        : pick(['LIVRE_PAYE', 'CLOTUREE', 'CLOTUREE'] as const)
  const items = Array.from({ length: int(1, 3) }, () => {
    const [nom, prix] = pick(OBJETS)
    const quantite = prix < 100 ? int(20, 250) : int(1, 6)
    return { nom, quantite, prixUnitaire: random() < 0.85 ? prix : null }
  })
  const prix = items.reduce((sum, item) => sum + item.quantite * (item.prixUnitaire ?? 500), 0)
  return {
    id: nextId(),
    createdAt: daysAgo(age),
    groupe: pick(GROUPES),
    type: random() < 0.45 ? 'ACHAT' : 'VENTE',
    status,
    statusLabel: STATUS_LABELS[status],
    prix: Math.round(prix * (0.9 + random() * 0.2) * 100) / 100,
    creator: member(int(0, 6)),
    items,
  }
})

// ---------------------------------------------------------------------------
// Cotisations

const MOTIFS = [
  'Vente de pièces au garage',
  'Livraison pour le Velvet',
  'Run de nuit au port',
  'Course de taxi',
  'Récolte au nord',
  'Revente de matériel',
  'Contrat sécurité',
  'Pourboire du club',
  'Remboursement avance',
  '',
]

export const COTISATIONS: CotisationEntry[] = []
for (let day = 0; day < 420; day += 1) {
  const perDay = day < 90 ? int(1, 4) : int(0, 2)
  for (let n = 0; n < perDay; n += 1) {
    const who = int(0, 13)
    const propre = random() < 0.75 ? int(4, 60) * 250 : 0
    const sale = random() < 0.45 ? int(2, 40) * 500 : 0
    if (propre + sale === 0) continue
    COTISATIONS.push({
      id: nextId(),
      date: parisDay(daysAgo(day)),
      member: member(who),
      propre,
      sale,
      description: pick(MOTIFS),
      declaredBy: random() < 0.15 ? member(int(0, 4)) : null,
    })
  }
}

export const objective: { value: Objective | null } = {
  value: { amount: 40_000, counts: 'total', reminder: true },
}

// ---------------------------------------------------------------------------
// Coffres

export const PROPERTIES: { id: string; name: string | null }[] = [
  { id: '8740', name: 'Planque du port' },
  { id: '1203', name: 'Entrepôt Elysian' },
  { id: '5521', name: null },
]

export const ITEMS: { item: string; label: string }[] = [
  { item: 'repair_kit', label: 'Kit de réparation' },
  { item: 'lockpick', label: 'Crochet' },
  { item: 'medikit', label: 'Trousse de soins' },
  { item: 'weapon_pistol', label: 'Pistolet' },
  { item: 'ammo_9mm', label: 'Munitions 9 mm' },
  { item: 'bandage', label: 'Bandage' },
]

export const CHARACTERS: { gameId: string; name: string }[] = [
  { gameId: '340772', name: 'Kyle Ian Duggan' },
  { gameId: '128844', name: 'Rosa Delgado' },
  { gameId: '559102', name: 'Marco Bellini' },
  { gameId: '771045', name: 'Ava Thornton' },
  { gameId: '402318', name: 'Dimitri Volkov' },
  { gameId: '995501', name: 'Joe Calloway' },
]

/** Personnage → membre Discord (index MEMBERS). */
export const LINKS = new Map<string, string>([
  ['340772', member(0).id],
  ['128844', member(1).id],
  ['559102', member(3).id],
  ['771045', member(5).id],
  ['402318', member(2).id],
])

export const MOVEMENTS: CoffreMovement[] = []
for (let index = 0; index < 340; index += 1) {
  const age = Math.pow(random(), 1.6) * 150
  const property = pick(PROPERTIES)
  const item = pick(ITEMS)
  const character = pick(CHARACTERS)
  const deposit = random() < 0.56
  const bulk = item.item === 'ammo_9mm' ? 10 : item.item === 'bandage' ? 3 : 1
  MOVEMENTS.push({
    id: nextId(),
    at: daysAgo(age),
    kind: deposit ? 'DEPOSIT' : 'WITHDRAW',
    propertyId: property.id,
    propertyName: property.name,
    item: item.item,
    itemLabel: item.label,
    amount: int(1, 8) * bulk,
    player: { gameId: character.gameId, name: character.name, member: null },
  })
}
MOVEMENTS.sort((a, b) => b.at.localeCompare(a.at))

export const coffreAlerts: { value: CoffreAlerts } = {
  value: {
    channelId: '1290000000000000001',
    roleId: null,
    thresholds: [
      { propertyId: null, item: 'medikit', min: 5 },
      { propertyId: '8740', item: 'ammo_9mm', min: 100 },
      { propertyId: '1203', item: 'repair_kit', min: 4 },
    ],
    bigWithdrawal: 50,
    unknownPlayer: true,
  },
}

// ---------------------------------------------------------------------------
// Contacts

type ContactSeed = Omit<Contact, 'id' | 'createdBy' | 'createdAt' | 'updatedAt' | 'notesCount'>

const CONTACT_SEEDS: ContactSeed[] = [
  {
    name: 'Famille Ortega',
    kind: 'GROUPE',
    relation: 'ALLIE',
    phone: '555-0142',
    description: 'Fournisseurs historiques, quartier sud. Paient toujours à l’heure.',
  },
  {
    name: 'Les Chacals',
    kind: 'GROUPE',
    relation: 'HOSTILE',
    phone: null,
    description: 'Ont tenté de braquer la planque du port en septembre. Ne pas négocier seul.',
  },
  {
    name: 'Atelier Mécano Sud',
    kind: 'ENTREPRISE',
    relation: 'ALLIE',
    phone: '555-0199',
    description: 'Réparations à prix d’ami, pièces moteur sur commande.',
  },
  {
    name: 'Club Le Velvet',
    kind: 'ENTREPRISE',
    relation: 'NEUTRE',
    phone: '555-0110',
    description: 'Gérante : Sofia. Lieu de rendez-vous neutre.',
  },
  {
    name: 'Groupe Kovač',
    kind: 'GROUPE',
    relation: 'NEUTRE',
    phone: null,
    description: 'Arrivés récemment, intéressés par les armes. À surveiller.',
  },
  {
    name: 'Dr Hélène Marsan',
    kind: 'PERSONNE',
    relation: 'ALLIE',
    phone: '555-0177',
    description: 'Soigne sans poser de questions. Contacter de nuit uniquement.',
  },
  { name: 'Taxi Downtown', kind: 'ENTREPRISE', relation: 'NEUTRE', phone: '555-0101', description: null },
  {
    name: 'Vinnie « le Rat »',
    kind: 'PERSONNE',
    relation: 'HOSTILE',
    phone: '555-0166',
    description: 'Indic présumé. Ne rien lui dire.',
  },
  {
    name: 'Marchand du port',
    kind: 'PERSONNE',
    relation: 'ALLIE',
    phone: '555-0123',
    description: 'Revend le matériel récupéré, 20 % de commission.',
  },
  {
    name: 'Los Santos Customs Nord',
    kind: 'ENTREPRISE',
    relation: 'NEUTRE',
    phone: '555-0150',
    description: 'Peinture et plaques.',
  },
  {
    name: 'Les Faucheurs',
    kind: 'GROUPE',
    relation: 'HOSTILE',
    phone: null,
    description: 'Gang de motards, territoire au nord de Sandy.',
  },
  {
    name: 'Jules Navarro',
    kind: 'PERSONNE',
    relation: 'NEUTRE',
    phone: '555-0188',
    description: 'Avocat. Coûteux mais efficace.',
  },
]

export const CONTACTS: Contact[] = CONTACT_SEEDS.map((seed, index) => {
  const createdAt = daysAgo(200 - index * 14)
  return {
    ...seed,
    id: String(index + 1),
    createdBy: member(int(0, 4)),
    createdAt,
    updatedAt: daysAgo(Math.max(1, 120 - index * 9)),
    notesCount: 0,
  }
})

const NOTE_TEXTS = [
  'Rencontre au Velvet, ambiance correcte. Prêts à un accord sur les munitions.',
  'Ont augmenté leurs prix de 15 %. À renégocier avant la prochaine commande.',
  'Deux de leurs membres repérés près de la planque hier soir.',
  'Paiement reçu en deux fois, tout est en ordre.',
  'Attention : leur chef a changé, nouveau contact à identifier.',
  'Ont proposé un échange de services contre des pièces moteur.',
  'Ne plus passer par Vinnie pour les contacts avec eux.',
]

export const NOTES = new Map<string, ContactNote[]>()
for (const contact of CONTACTS) {
  const count = int(0, 3)
  const notes = Array.from({ length: count }, (_, index) => ({
    id: nextId(),
    author: member(int(0, 4)),
    body: pick(NOTE_TEXTS),
    createdAt: daysAgo(index * 11 + int(1, 10)),
  })).sort((a, b) => b.createdAt.localeCompare(a.createdAt))
  NOTES.set(contact.id, notes)
  contact.notesCount = notes.length
}

// ---------------------------------------------------------------------------
// Journal du staff

export const JOURNAL_ACTIONS: { key: string; label: string }[] = [
  { key: 'cotisation.autrui', label: 'Cotisation déclarée pour un membre' },
  { key: 'cotisation.suppression', label: 'Cotisation supprimée' },
  { key: 'presence.forcer', label: 'Réponse de présence forcée' },
  { key: 'recrue.veto', label: 'Véto posé' },
  { key: 'recrue.ajout', label: 'Recrue ajoutée' },
  { key: 'contact.modification', label: 'Contact modifié' },
  { key: 'coffre.lien', label: 'Personnage relié' },
  { key: 'transaction.suppression', label: 'Transaction supprimée' },
  { key: 'absence.autrui', label: 'Absence déclarée pour un membre' },
]

const DETAILS: Record<string, () => string> = {
  'cotisation.autrui': () =>
    `<p><strong>${int(2, 30) * 500} $</strong> propre · motif : <em>${pick(MOTIFS) || 'sans motif'}</em></p>`,
  'cotisation.suppression': () => `<p>Doublon de la déclaration du ${int(1, 28)}/${int(8, 10)}.</p>`,
  'presence.forcer': () => `<p>Présence <strong>Soirée au port</strong> : réponse passée à <code>RETARD</code>.</p>`,
  'recrue.veto': () => `<p>Motif : <span class="spoiler" tabindex="0">a parlé de la planque à un inconnu</span></p>`,
  'recrue.ajout': () => '<p>Période d’essai de 2 semaines.</p>',
  'contact.modification': () => '<p>Relation : <strong>Neutre</strong> → <strong>Hostile</strong></p>',
  'coffre.lien': () => `<p>Personnage <code>${pick(CHARACTERS).name}</code> relié.</p>`,
  'transaction.suppression': () => `<p>Transaction avec <strong>${pick(GROUPES)}</strong> (saisie en double).</p>`,
  'absence.autrui': () => '<p>Du 18 au 22 octobre · <em>vacances</em></p>',
}

export const JOURNAL: JournalEntry[] = Array.from({ length: 74 }, (_, index) => {
  const action = pick(JOURNAL_ACTIONS)
  const withTarget = !['transaction.suppression', 'contact.modification'].includes(action.key)
  return {
    id: nextId(),
    at: daysAgo(index * 1.3),
    actor: member(int(0, 4)),
    action: action.key,
    label: action.label,
    target: withTarget ? member(int(3, 15)) : null,
    detailsHtml: DETAILS[action.key]?.() ?? '',
  }
})

// ---------------------------------------------------------------------------
// Tickets

const TICKET_USERS = ['Elodie', 'Karim_RP', 'Mathéo.B', 'Lou', 'Rayan', 'Clara M.', 'Nathan', 'Zoé', 'Ilyes', 'Margaux']
const STAFF = ['Marco', 'Inès', 'Hugo', 'Sacha']

export const TICKETS: TicketRow[] = Array.from({ length: 63 }, (_, index) => {
  const status: TicketStatus = index < 4 ? 'OPEN' : index === 5 ? 'SUSPENDED' : 'CLOSED'
  return {
    id: `${1296000000000000000n + BigInt(index * 3301)}`,
    number: 512 - index,
    user: { id: `${4100000000000000000n + BigInt(index * 97)}`, name: pick(TICKET_USERS) },
    status,
    openedAt: daysAgo(index * 0.9, int(9, 23)),
    exchanges: status === 'OPEN' && index === 0 ? 1 : int(2, 24),
    claimedBy: random() < 0.6 ? pick(STAFF) : null,
  }
})

const CAPTURE =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='320' height='180'%3E%3Crect width='320' height='180' fill='%23313338'/%3E%3Crect x='16' y='16' width='120' height='12' rx='6' fill='%235865f2'/%3E%3Crect x='16' y='44' width='260' height='10' rx='5' fill='%23949ba4'/%3E%3Crect x='16' y='64' width='200' height='10' rx='5' fill='%23949ba4'/%3E%3Ctext x='16' y='150' fill='%23f23f43' font-family='sans-serif' font-size='16'%3EAcc%C3%A8s refus%C3%A9 au salon%3C/text%3E%3C/svg%3E"

export function ticketLogEntries(openedAt: string): LogEntry[] {
  const start = new Date(openedAt).getTime()
  const at = (minutes: number) => new Date(start + minutes * 60_000).toISOString()
  const entry = (
    partial: Partial<LogEntry> & Pick<LogEntry, 'kind' | 'author' | 'bodyHtml'>,
    minutes: number,
  ): LogEntry => ({
    id: nextId(),
    newDay: false,
    role: null,
    badge: null,
    number: null,
    at: at(minutes),
    monospace: false,
    edit: null,
    images: [],
    files: [],
    embeds: null,
    technical: `message ${nextId()} · auteur ${nextId()}`,
    ...partial,
  })
  const entries: LogEntry[] = [
    entry(
      {
        kind: 'system',
        author: 'Modmail',
        badge: 'Bot',
        bodyHtml:
          'ACCOUNT AGE <strong>3 years, 2 months</strong>, ID <strong>106391128718245888</strong><br>NICKNAME <strong>Elodie</strong>, JOINED <strong>8 months</strong> ago',
        newDay: true,
      },
      0,
    ),
    entry(
      {
        kind: 'user',
        author: 'Elodie',
        badge: 'Utilisateur',
        bodyHtml:
          'Bonjour, je n’arrive plus à accéder au salon <span class="mention">#planque-du-port</span> depuis hier soir.',
      },
      1,
    ),
    entry(
      {
        kind: 'staff',
        author: 'Marco',
        role: 'Modération',
        badge: 'Staff',
        number: 1,
        bodyHtml:
          'Bonjour ! Peux-tu nous envoyer une capture ? Règlement : <a href="https://example.com/regles" target="_blank" rel="noopener noreferrer">https://example.com/regles</a>',
      },
      4,
    ),
    entry(
      {
        kind: 'user',
        author: 'Elodie',
        badge: 'Utilisateur',
        bodyHtml: 'Voilà la capture 👇',
        images: [CAPTURE],
        files: [{ url: '#rapport.pdf', name: 'rapport.pdf' }],
      },
      6,
    ),
    entry(
      {
        kind: 'user',
        author: 'Elodie',
        badge: 'Utilisateur',
        bodyHtml:
          '<h3>Récapitulatif</h3><blockquote><p>Rôle perdu <strong>pendant la maintenance</strong> du <time class="timestamp" datetime="' +
          at(-60 * 20) +
          '" data-local data-format="D"></time> (<time class="timestamp" datetime="' +
          at(-60 * 20) +
          '" data-local data-format="R"></time>)</p></blockquote><ul><li>pseudo <code>Elodie#0001</code></li><li><s>ancien</s> <u>nouveau</u> <span class="spoiler" tabindex="0">mot de passe du coffre</span></li></ul><p class="subtext">envoyé depuis mobile</p>',
      },
      7,
    ),
    entry(
      {
        kind: 'internal',
        author: 'Marco',
        badge: 'Note interne',
        bodyHtml: 'Vu, c’est le rôle qui a sauté pendant la maintenance. Je regarde demain.',
      },
      9,
    ),
    entry({ kind: 'command', author: 'Marco', badge: 'Commande', bodyHtml: '!close 12h', monospace: true }, 10),
    entry(
      {
        kind: 'system',
        author: 'Marco',
        badge: 'Réponse modifiée',
        bodyHtml: '',
        newDay: true,
        edit: { beforeHtml: 'On regarde ça demain.', afterHtml: 'On regarde ça ce matin, merci de ta patience !' },
      },
      60 * 11,
    ),
    entry(
      {
        kind: 'staff',
        author: 'Inès',
        role: 'Administration',
        badge: 'Staff · anonyme',
        number: 2,
        bodyHtml: 'C’est corrigé : tu as de nouveau le rôle <span class="mention">@Membre</span>. Bon jeu !',
        embeds: 'Embed : « Rôle rétabli » — Membre',
      },
      60 * 11 + 12,
    ),
    entry({ kind: 'danger', author: 'Marco', badge: 'Réponse supprimée', bodyHtml: 'Test, à ignorer.' }, 60 * 11 + 14),
    entry({ kind: 'system', author: 'Modmail', badge: 'Bot', bodyHtml: 'Closing thread…' }, 60 * 11 + 30),
  ]
  return entries
}

// ---------------------------------------------------------------------------
// Présences, absences, recrues, véhicules

export const PRESENCE_TITLES = [
  'Soirée au port',
  'Réunion du dimanche',
  'Run Elysian',
  'Rendez-vous Velvet',
  'Entraînement tir',
]

export const VEHICLES: Record<number, { plate: string; model: string | null }[]> = {
  0: [
    { plate: '42 CRB 13', model: 'Sultan RS' },
    { plate: '07 PRT 88', model: 'Speedo' },
  ],
  1: [{ plate: '19 INS 02', model: 'Buffalo STX' }],
  3: [{ plate: '55 HDL 31', model: null }],
}

export const PHONES: Record<number, string> = {
  0: '555-4217',
  1: '555-1934',
  2: '555-7720',
  3: '555-0458',
  5: '555-3311',
}
