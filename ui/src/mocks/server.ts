// Faux serveur de l'API (`VITE_MOCK=1`) : répond à toutes les routes du contrat, écritures gardées en mémoire.
// Scénarios de connexion : `?mock=anon` (pas connecté), `?mock=aucun` (aucun accès), `?mock=membre`
// (simple membre), `?mock=admin` (par défaut) ; le choix est mémorisé dans sessionStorage.

import type {
  CoffreAlerts,
  CoffreCorrection,
  CoffreInventory,
  CoffreInventoryResult,
  CoffreKind,
  CoffreMeta,
  CoffreMovement,
  CoffrePlayer,
  CoffreSeries,
  CoffreStock,
  ConfigStatus,
  Contact,
  ContactInput,
  ContactKind,
  ContactNote,
  ContactsResponse,
  CotisationEntry,
  CotisationsResponse,
  GuildAccess,
  GuildStats,
  JournalResponse,
  Level,
  Me,
  MemberProfile,
  MemberRef,
  NewCotisation,
  Objective,
  ObjectiveProgress,
  Page,
  Perm,
  PermissionsUpdate,
  Relation,
  TicketLog,
  TicketStats,
  TicketsResponse,
  TransactionStatus,
  TransactionsResponse,
  WeeklyAmount,
} from '@/api/types'
import type { GuildHealth, GuildOverview } from '@/api/types-site'
import {
  fallbackItemLabel,
  isPropertyId,
  MAX_INVENTORY,
  MAX_INVENTORY_ITEMS,
  normalizeItem,
} from '@/lib/coffre-inventory'

import {
  ACTIONS,
  addDays,
  CHARACTERS,
  COTISATIONS,
  CONTACTS,
  coffreAlerts,
  daysAgo,
  GUILD_B_ID,
  GUILDS,
  int,
  ITEMS,
  JOURNAL,
  JOURNAL_ACTIONS,
  LINKS,
  ME,
  MEMBERS,
  member,
  memberRoles,
  mondayOf,
  MOVEMENTS,
  nextId,
  NOTES,
  NOW,
  objective,
  parisDay,
  permissionLevels,
  permissionRoles,
  permissionsResponse,
  PHONES,
  PRESENCE_TITLES,
  PROPERTIES,
  random,
  STATUS_LABELS,
  TICKETS,
  ticketLogEntries,
  TRANSACTIONS,
  VEHICLES,
} from './fixtures'
import { registerGroupeRoutes } from './groupe'

class HttpError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'HttpError'
    this.status = status
  }
}

type Scenario = 'admin' | 'membre' | 'aucun' | 'anon'

const valid = (value: string | null): value is Scenario =>
  value === 'admin' || value === 'membre' || value === 'aucun' || value === 'anon'

const scenario = (): Scenario => {
  const fromUrl = new URL(globalThis.location.href).searchParams.get('mock')
  try {
    if (valid(fromUrl)) sessionStorage.setItem('mock', fromUrl)
    const stored = sessionStorage.getItem('mock')
    if (valid(stored)) return stored
  } catch {
    if (valid(fromUrl)) return fromUrl
  }
  return 'admin'
}

const MEMBER_PERMS: Perm[] = [
  'presence.creer',
  'presence.stats',
  'transaction.utiliser',
  'vehicule.utiliser',
  'cotisation.ajouter',
  'contact.voir',
  'contact.ajouter',
  'coffre.voir',
]

const guildsFor = (current: Scenario): GuildAccess[] => {
  if (current === 'aucun' || current === 'anon') return []
  if (current === 'membre') return GUILDS.map((guild) => ({ ...guild, isAdmin: false, perms: MEMBER_PERMS }))
  return GUILDS
}

const me = (): Me => {
  const current = scenario()
  return {
    user: current === 'anon' ? null : { id: ME.id, name: ME.name, avatarUrl: null },
    loginEnabled: true,
    // En maquette, « se connecter » bascule sur le scénario administrateur.
    loginUrl: '/?mock=admin',
    tickets: current === 'admin',
    guilds: guildsFor(current),
  }
}

const requireGuild = (guildId: string, perm?: Perm | 'admin'): GuildAccess => {
  const current = scenario()
  if (current === 'anon') throw new HttpError(401, 'Connexion requise.')
  const guild = guildsFor(current).find((item) => item.id === guildId)
  if (!guild) throw new HttpError(404, 'Serveur introuvable.')
  if (perm === 'admin' ? !guild.isAdmin : perm && !guild.perms.includes(perm)) {
    throw new HttpError(403, "Votre rôle ne permet pas d'accéder à cette page.")
  }
  if (guildId === GUILD_B_ID && perm !== undefined && perm !== 'contact.voir' && perm !== 'contact.ajouter') {
    // Le second serveur n'a presque rien de configuré.
    throw new HttpError(403, "Votre rôle ne permet pas d'accéder à cette page.")
  }
  return guild
}

const paginate = <T>(items: T[], page: number, pageSize = 25): Page<T> => {
  const safe = Math.max(1, page)
  return { items: items.slice((safe - 1) * pageSize, safe * pageSize), total: items.length, page: safe, pageSize }
}

const memberById = (id: string): MemberRef => {
  const found = MEMBERS.find((item) => item.id === id)
  if (!found) throw new HttpError(404, 'Membre introuvable sur ce serveur.')
  return found
}

const memberIndex = (id: string): number => MEMBERS.findIndex((item) => item.id === id)

/** Les recrues de la maquette n'ont pas le rôle membre : rien n'est enregistré pour elles. */
const requireMemberRole = (person: MemberRef) => {
  if (memberIndex(person.id) >= 14) {
    throw new HttpError(422, `${person.name} n'a pas le rôle membre : rien n'est enregistré pour cette personne.`)
  }
}

const today = (): string => parisDay(NOW)

const sinceDay = (days: number | null): string | null => (days === null ? null : addDays(today(), -days + 1))

const parseDays = (raw: string | null, fallback: number | null): number | null => {
  if (raw === 'tout') return null
  const value = Number(raw)
  return raw && Number.isInteger(value) && value > 0 && value <= 3650 ? value : fallback
}

// ---------------------------------------------------------------------------
// Statistiques

const weeks = (count: number, end = today()): string[] => {
  const last = mondayOf(end)
  return Array.from({ length: count }, (_, index) => addDays(last, (index - count + 1) * 7))
}

let statsCache: GuildStats | null = null

const stats = (): GuildStats => {
  statsCache ??= buildStats()
  return statsCache
}

const buildStats = (): GuildStats => {
  const attendance = MEMBERS.slice(0, 14).map((ref, index) => {
    const present = int(12, 30) - (index % 5) * 2
    const retard = int(0, 6)
    const absent = int(0, 10) + (index > 10 ? 6 : 0)
    const total = Math.max(1, present + retard + absent)
    return {
      member: ref,
      present: Math.max(0, present),
      retard,
      absent,
      percent: Math.round(((Math.max(0, present) + retard) / total) * 100),
    }
  })
  attendance.sort((a, b) => b.percent - a.percent)
  const attendanceWeekly = weeks(12).map((weekStart, index) => ({
    weekStart,
    rate: index === 3 ? null : Math.round(62 + Math.sin(index / 2) * 10 + random() * 12),
    presences: index === 3 ? 0 : int(2, 5),
  }))
  return {
    figures: [
      { key: 'presences', label: 'Présences clôturées', value: '148', hint: 'depuis le début' },
      { key: 'taux', label: 'Taux de présence', value: '78 %', hint: 'moyenne, 12 semaines' },
      { key: 'membres', label: 'Membres suivis', value: '14', hint: null },
      { key: 'absences', label: 'Absences en cours', value: '2', hint: null },
      { key: 'transactions', label: 'Transactions en cours', value: '4', hint: '3 ventes, 1 achat' },
      { key: 'vehicules', label: 'Véhicules', value: '11', hint: 'enregistrés' },
    ],
    attendance,
    attendanceWeekly,
    recruits: [
      { member: member(14), pour: 5, contre: 1, blanc: 2, veto: null },
      {
        member: member(15),
        pour: 2,
        contre: 3,
        blanc: 1,
        veto: { by: member(1), motif: 'A parlé de la planque à un inconnu.' },
      },
    ],
    absencesCurrent: [
      { member: member(6), from: addDays(today(), -2), to: addDays(today(), 4), motif: 'Vacances' },
      { member: member(9), from: addDays(today(), -1), to: addDays(today(), 1), motif: 'Examens' },
    ],
    tickets: ticketStats(),
  }
}

const ticketStats = (): TicketStats => ({
  open: 4,
  opened: 41,
  closed: 38,
  unanswered: 1,
  medianFirstResponse: '12 min',
  staff: [
    { name: 'Marco', replies: 64, tickets: 22, claims: 14, firstResponses: 15, averageFirstResponse: '9 min' },
    { name: 'Inès', replies: 51, tickets: 19, claims: 11, firstResponses: 12, averageFirstResponse: '14 min' },
    { name: 'Hugo', replies: 23, tickets: 9, claims: 5, firstResponses: 6, averageFirstResponse: '31 min' },
    { name: 'Sacha', replies: 7, tickets: 4, claims: 1, firstResponses: 2, averageFirstResponse: null },
  ],
})

// ---------------------------------------------------------------------------
// Transactions

const transactions = (params: URLSearchParams): TransactionsResponse => {
  const groupe = params.get('groupe')?.toLowerCase() ?? ''
  const statut = params.get('statut') ?? ''
  const type = params.get('type') ?? ''
  const du = params.get('du') ?? ''
  const au = params.get('au') ?? ''
  const list = TRANSACTIONS.filter((item) => {
    const day = parisDay(item.createdAt)
    return (
      (!groupe || item.groupe.toLowerCase().includes(groupe)) &&
      (!statut || item.status === statut) &&
      (!type || item.type === type) &&
      (!du || day >= du) &&
      (!au || day <= au)
    )
  })
  const counted = list.filter((item) => item.status !== 'ANNULEE')
  const sum = (kind: 'ACHAT' | 'VENTE') =>
    counted.filter((item) => item.type === kind).reduce((total, item) => total + item.prix, 0)
  const months = new Map<string, { month: string; achats: number; ventes: number; count: number }>()
  for (const item of list) {
    const month = parisDay(item.createdAt).slice(0, 7)
    const row = months.get(month) ?? { month, achats: 0, ventes: 0, count: 0 }
    row.count += 1
    if (item.status !== 'ANNULEE') {
      if (item.type === 'ACHAT') row.achats += item.prix
      else row.ventes += item.prix
    }
    months.set(month, row)
  }
  const achats = sum('ACHAT')
  const ventes = sum('VENTE')
  return {
    totals: {
      count: list.length,
      achats,
      ventes,
      solde: ventes - achats,
      inProgress: list.filter((item) =>
        ['PREPARATION', 'EN_ATTENTE_RDV', 'AVANCE_ATTENTE_PAIEMENT'].includes(item.status),
      ).length,
    },
    months: [...months.values()].toSorted((a, b) => b.month.localeCompare(a.month)).slice(0, 12),
    statuses: (Object.keys(STATUS_LABELS) as TransactionStatus[]).map((value) => ({
      value,
      label: STATUS_LABELS[value],
    })),
    list: paginate(list, Number(params.get('page') ?? 1)),
  }
}

// ---------------------------------------------------------------------------
// Cotisations

const objectiveAmount = (entry: { propre: number; sale: number }, counts: Objective['counts']): number =>
  counts === 'propre' ? entry.propre : counts === 'sale' ? entry.sale : entry.propre + entry.sale

const objectiveProgress = (onlyMember?: string): ObjectiveProgress[] => {
  const current = objective.value
  if (!current) return []
  const monday = mondayOf(today())
  return MEMBERS.slice(0, 16)
    .filter((ref) => !onlyMember || ref.id === onlyMember)
    .map((ref, index) => {
      const amount = COTISATIONS.filter((entry) => entry.member.id === ref.id && entry.date >= monday).reduce(
        (sum, entry) => sum + objectiveAmount(entry, current.counts),
        0,
      )
      return {
        member: ref,
        amount,
        target: current.amount,
        reached: amount >= current.amount,
        exempt: index === 6 || index >= 14,
      }
    })
    .toSorted((a, b) => a.amount / a.target - b.amount / b.target)
}

const weeklyOf = (entries: CotisationEntry[], count: number): WeeklyAmount[] =>
  weeks(count).map((weekStart) => {
    const end = addDays(weekStart, 7)
    const inWeek = entries.filter((entry) => entry.date >= weekStart && entry.date < end)
    return {
      weekStart,
      propre: inWeek.reduce((sum, entry) => sum + entry.propre, 0),
      sale: inWeek.reduce((sum, entry) => sum + entry.sale, 0),
    }
  })

const cotisationsFiltered = (params: URLSearchParams): { entries: CotisationEntry[]; days: number | null } => {
  const days = parseDays(params.get('jours'), null)
  const since = sinceDay(days)
  const membre = params.get('membre')
  const entries = COTISATIONS.filter(
    (entry) => (!since || entry.date >= since) && (!membre || entry.member.id === membre),
  ).toSorted((a, b) => b.date.localeCompare(a.date))
  return { entries, days }
}

const cotisations = (params: URLSearchParams): CotisationsResponse => {
  const { entries, days } = cotisationsFiltered(params)
  const byMember = new Map<string, { member: MemberRef; propre: number; sale: number; count: number }>()
  for (const entry of entries) {
    const row = byMember.get(entry.member.id) ?? { member: entry.member, propre: 0, sale: 0, count: 0 }
    row.propre += entry.propre
    row.sale += entry.sale
    row.count += 1
    byMember.set(entry.member.id, row)
  }
  const weekCount = days === null ? 12 : Math.min(53, Math.max(1, Math.ceil(days / 7)))
  return {
    totals: {
      count: entries.length,
      propre: entries.reduce((sum, entry) => sum + entry.propre, 0),
      sale: entries.reduce((sum, entry) => sum + entry.sale, 0),
    },
    members: [...byMember.values()].toSorted((a, b) => b.propre + b.sale - (a.propre + a.sale)),
    weekly: weeklyOf(entries, weekCount),
    entries: paginate(entries, Number(params.get('page') ?? 1)),
    objective: objective.value,
    objectiveProgress: objectiveProgress(),
  }
}

const cotisationsCsv = (params: URLSearchParams): string => {
  const { entries } = cotisationsFiltered(params)
  const lines = entries.map((entry) =>
    [entry.date, entry.member.name, entry.propre, entry.sale, entry.description, entry.declaredBy?.name ?? ''].join(
      ';',
    ),
  )
  return ['Date;Membre;Propre;Sale;Description;Déclarée par', ...lines].join('\n')
}

const addCotisation = (body: NewCotisation): CotisationEntry => {
  const propre = Math.trunc(body.propre)
  const sale = Math.trunc(body.sale)
  if (propre < 0 || sale < 0) throw new HttpError(422, 'Les montants ne peuvent pas être négatifs.')
  if (propre + sale <= 0) throw new HttpError(422, 'Indiquez au moins un montant propre ou sale.')
  if (body.description.length > 200) throw new HttpError(422, 'La description dépasse 200 caractères.')
  const date = body.date ?? today()
  if (date > today()) throw new HttpError(422, 'La date ne peut pas être dans le futur.')
  const target = body.memberId ? memberById(body.memberId) : ME
  requireMemberRole(target)
  const entry: CotisationEntry = {
    id: nextId(),
    date,
    member: target,
    propre,
    sale,
    description: body.description.trim(),
    declaredBy: target.id === ME.id ? null : ME,
  }
  COTISATIONS.unshift(entry)
  return entry
}

// ---------------------------------------------------------------------------
// Coffres

const linkedMember = (gameId: string | null): MemberRef | null => {
  const id = gameId ? LINKS.get(gameId) : undefined
  return id ? memberById(id) : null
}

const withLinks = (movement: CoffreMovement): CoffreMovement => {
  const property = PROPERTIES.find((item) => item.id === movement.propertyId)
  return {
    ...movement,
    propertyName: property?.name ?? null,
    // Une correction garde son auteur ; un mouvement du jeu prend le membre relié au personnage.
    player:
      movement.kind === 'ADJUST'
        ? movement.player
        : { ...movement.player, member: linkedMember(movement.player.gameId) },
  }
}

const KIND_FILTERS: Record<string, CoffreKind | undefined> = {
  deposit: 'DEPOSIT',
  withdraw: 'WITHDRAW',
  adjust: 'ADJUST',
  correction: 'ADJUST',
}

/** Somme des mouvements d'un type ; dépôts − retraits + corrections pour `stock`. */
const sumOf = (moves: readonly CoffreMovement[], kind: CoffreKind) =>
  moves.filter((move) => move.kind === kind).reduce((sum, move) => sum + move.amount, 0)

const stockOf = (moves: readonly CoffreMovement[]) =>
  sumOf(moves, 'DEPOSIT') - sumOf(moves, 'WITHDRAW') + sumOf(moves, 'ADJUST')

const filteredMovements = (params: URLSearchParams): CoffreMovement[] => {
  const property = params.get('propriete')
  const item = params.get('objet')
  const player = params.get('joueur')
  const type = params.get('type')
  const days = parseDays(params.get('jours'), 30)
  const since = days === null ? null : NOW - days * 86_400_000
  return MOVEMENTS.filter(
    (movement) =>
      (!property || movement.propertyId === property) &&
      (!item || movement.item === item) &&
      (!player || movement.player.gameId === player) &&
      (!type || movement.kind === KIND_FILTERS[type]) &&
      (since === null || new Date(movement.at).getTime() >= since),
  ).map((movement) => withLinks(movement))
}

const coffreMeta = (): CoffreMeta => ({
  properties: PROPERTIES,
  items: ITEMS,
  links: [...LINKS.entries()].map(([gameId, memberId]) => ({
    gameId,
    name: CHARACTERS.find((item) => item.gameId === gameId)?.name ?? gameId,
    member: memberById(memberId),
  })),
  channels: [
    { channelId: '1290000000000000010', name: 'logs-coffre-port', webhookKnown: true },
    { channelId: '1290000000000000011', name: 'logs-coffre-elysian', webhookKnown: true },
    { channelId: '1290000000000000012', name: null, webhookKnown: false },
  ],
})

const coffreStock = (params: URLSearchParams): CoffreStock => {
  const only = params.get('propriete')
  const { thresholds } = coffreAlerts.value
  return {
    properties: PROPERTIES.filter((property) => !only || property.id === only).map((property) => ({
      id: property.id,
      name: property.name,
      items: ITEMS.map((item) => {
        const moves = MOVEMENTS.filter((move) => move.propertyId === property.id && move.item === item.item)
        const quantity = stockOf(moves)
        const threshold = thresholds.find(
          (rule) => rule.item === item.item && (rule.propertyId === null || rule.propertyId === property.id),
        )
        return {
          ...item,
          deposited: sumOf(moves, 'DEPOSIT'),
          withdrawn: sumOf(moves, 'WITHDRAW'),
          adjusted: sumOf(moves, 'ADJUST'),
          quantity,
          low: quantity < 0 || (threshold !== undefined && quantity < threshold.min),
          seen: moves.length > 0,
        }
      })
        .filter((item) => item.seen)
        .map(({ seen: _seen, ...item }) => item),
    })),
  }
}

const coffrePlayers = (params: URLSearchParams): CoffrePlayer[] => {
  const players = new Map<string, CoffrePlayer>()
  // Les corrections d'inventaire ne sont pas l'activité d'un joueur.
  for (const movement of filteredMovements(params).filter((move) => move.kind !== 'ADJUST')) {
    const key = movement.player.gameId ?? movement.player.name
    const row = players.get(key) ?? {
      gameId: movement.player.gameId,
      name: movement.player.name,
      member: movement.player.member,
      deposited: 0,
      withdrawn: 0,
      count: 0,
    }
    if (movement.kind === 'DEPOSIT') row.deposited += movement.amount
    else row.withdrawn += movement.amount
    row.count += 1
    players.set(key, row)
  }
  return [...players.values()].toSorted((a, b) => b.count - a.count)
}

const coffreSeries = (params: URLSearchParams): CoffreSeries => {
  const item = params.get('objet')
  if (!item) throw new HttpError(400, 'Choisissez un objet.')
  const property = params.get('propriete')
  const days = parseDays(params.get('jours'), 30)
  const moves = MOVEMENTS.filter((move) => move.item === item && (!property || move.propertyId === property))
  const first = moves.at(-1)
  const start = days === null ? (first ? parisDay(first.at) : today()) : addDays(today(), -days + 1)
  const series: CoffreSeries = []
  let stock = stockOf(moves.filter((move) => parisDay(move.at) < start))
  for (let day = start; day <= today(); day = addDays(day, 1)) {
    const inDay = moves.filter((move) => parisDay(move.at) === day)
    stock += stockOf(inDay)
    series.push({
      day,
      deposited: sumOf(inDay, 'DEPOSIT'),
      withdrawn: sumOf(inDay, 'WITHDRAW'),
      adjusted: sumOf(inDay, 'ADJUST'),
      stock,
    })
  }
  return series
}

const labelOfItem = (item: string) => ITEMS.find((entry) => entry.item === item)?.label ?? fallbackItemLabel(item)

/** Inventaire : mêmes contrôles et même calcul que le bot (écarts → mouvements `ADJUST` signés). */
const coffreInventory = (body: CoffreInventory): CoffreInventoryResult => {
  const propertyId = String(body.propertyId ?? '').trim()
  if (!isPropertyId(propertyId)) throw new HttpError(422, 'Propriété invalide : son numéro, ex : 8740.')
  if (!Array.isArray(body.items) || body.items.length === 0 || body.items.length > MAX_INVENTORY_ITEMS) {
    throw new HttpError(422, `Entre 1 et ${MAX_INVENTORY_ITEMS} objets par inventaire.`)
  }
  const counted = new Map<string, number>()
  for (const line of body.items) {
    const item = normalizeItem(String(line.item))
    if (!item) throw new HttpError(422, `Objet invalide : « ${line.item} ».`)
    if (!Number.isInteger(line.quantity) || line.quantity < 0 || line.quantity > MAX_INVENTORY) {
      throw new HttpError(422, `${labelOfItem(item)} : quantité entre 0 et ${MAX_INVENTORY}.`)
    }
    counted.set(item, line.quantity)
  }
  const property = PROPERTIES.find((entry) => entry.id === propertyId)
  const corrections: CoffreCorrection[] = []
  for (const [item, after] of counted) {
    const before = stockOf(MOVEMENTS.filter((move) => move.propertyId === propertyId && move.item === item))
    if (after === before) continue
    if (!ITEMS.some((entry) => entry.item === item)) ITEMS.push({ item, label: labelOfItem(item) })
    MOVEMENTS.unshift({
      id: nextId(),
      at: new Date().toISOString(),
      kind: 'ADJUST',
      propertyId,
      propertyName: property?.name ?? null,
      item,
      itemLabel: labelOfItem(item),
      amount: after - before,
      player: { gameId: null, name: ME.name, member: ME },
    })
    corrections.push({ item, label: labelOfItem(item), before, after })
  }
  // Une nouvelle propriété apparaît dès sa première correction.
  if (!property && corrections.length > 0) PROPERTIES.push({ id: propertyId, name: null })
  return { corrections }
}

// ---------------------------------------------------------------------------
// Contacts

const KINDS = new Set<ContactKind>(['GROUPE', 'ENTREPRISE', 'PERSONNE'])
const RELATIONS = new Set<Relation>(['ALLIE', 'NEUTRE', 'HOSTILE'])

const validateContact = (input: Partial<ContactInput>, partial: boolean): void => {
  if (!partial || input.name !== undefined) {
    const name = input.name?.trim() ?? ''
    if (!name) throw new HttpError(422, 'Le nom est obligatoire.')
    if (name.length > 80) throw new HttpError(422, 'Le nom dépasse 80 caractères.')
    const duplicate = CONTACTS.find((contact) => contact.name.toLowerCase() === name.toLowerCase())
    if (duplicate && !partial) throw new HttpError(422, `Un contact « ${duplicate.name} » existe déjà.`)
  }
  if (input.kind !== undefined && !KINDS.has(input.kind)) throw new HttpError(422, 'Type inconnu.')
  if (input.relation !== undefined && !RELATIONS.has(input.relation)) throw new HttpError(422, 'Relation inconnue.')
  if (input.phone && input.phone.length > 30) throw new HttpError(422, 'Le téléphone dépasse 30 caractères.')
  if (input.description && input.description.length > 500) {
    throw new HttpError(422, 'La description dépasse 500 caractères.')
  }
}

const countRelation = (value: Relation) => CONTACTS.filter((contact) => contact.relation === value).length

const contactsList = (params: URLSearchParams, canNotes: boolean): ContactsResponse => {
  const relation = params.get('relation')?.toUpperCase()
  const kind = params.get('type')?.toUpperCase()
  const q = params.get('q')?.toLowerCase().trim()
  const items = CONTACTS.filter(
    (contact) =>
      (!relation || contact.relation === relation) &&
      (!kind || contact.kind === kind) &&
      (!q || contact.name.toLowerCase().includes(q) || (contact.description ?? '').toLowerCase().includes(q)),
  )
    .map((contact) => ({ ...contact, notesCount: canNotes ? (NOTES.get(contact.id)?.length ?? 0) : null }))
    .toSorted((a, b) => a.name.localeCompare(b.name, 'fr'))
  return {
    counts: { allie: countRelation('ALLIE'), neutre: countRelation('NEUTRE'), hostile: countRelation('HOSTILE') },
    items,
  }
}

const findContact = (id: string): Contact => {
  const contact = CONTACTS.find((item) => item.id === id)
  if (!contact) throw new HttpError(404, 'Contact introuvable.')
  return contact
}

// ---------------------------------------------------------------------------
// Fiche membre

const profile = (userId: string): MemberProfile => {
  const ref = memberById(userId)
  const index = memberIndex(userId)
  const recrue = index >= 14
  const own = COTISATIONS.filter((entry) => entry.member.id === userId)
  const characters = [...LINKS.entries()]
    .filter(([, memberId]) => memberId === userId)
    .map(([gameId]) => ({ gameId, name: CHARACTERS.find((item) => item.gameId === gameId)?.name ?? gameId }))
  const moves = MOVEMENTS.filter((move) => characters.some((character) => character.gameId === move.player.gameId))
  const statuses = ['PRESENT', 'PRESENT', 'PRESENT', 'RETARD', 'ABSENT', 'NON_REPONDU'] as const
  const recent = [...PRESENCE_TITLES, ...PRESENCE_TITLES].slice(0, 8).map((title, position) => ({
    id: nextId(),
    title,
    date: daysAgo(position * 3 + 1),
    status: statuses[(position + index) % statuses.length] ?? 'PRESENT',
  }))
  return {
    member: ref,
    joinedAt: daysAgo(400 - index * 20),
    roles: memberRoles(index),
    presence: recrue
      ? null
      : { present: 24 - index, retard: 3 + (index % 3), absent: 2 + (index % 4), rate: 88 - index * 2, recent },
    absences:
      index === 6
        ? [
            {
              from: `${addDays(today(), -2)}T08:00:00Z`,
              to: `${addDays(today(), 4)}T21:59:00Z`,
              motif: 'Vacances',
              current: true,
            },
          ]
        : index === 0
          ? [
              {
                from: `${addDays(today(), -40)}T08:00:00Z`,
                to: `${addDays(today(), -36)}T21:59:00Z`,
                motif: 'Déménagement',
                current: false,
              },
            ]
          : [],
    cotisations: recrue
      ? null
      : {
          totals: {
            count: own.length,
            propre: own.reduce((sum, entry) => sum + entry.propre, 0),
            sale: own.reduce((sum, entry) => sum + entry.sale, 0),
          },
          weekly: weeklyOf(own, 12),
          recent: own.slice(0, 6),
          objective: objectiveProgress(userId)[0] ?? null,
        },
    coffres:
      characters.length === 0
        ? null
        : {
            characters,
            deposited: moves.filter((move) => move.kind === 'DEPOSIT').reduce((sum, move) => sum + move.amount, 0),
            withdrawn: moves.filter((move) => move.kind === 'WITHDRAW').reduce((sum, move) => sum + move.amount, 0),
            recent: moves.slice(0, 8).map((movement) => withLinks(movement)),
          },
    recrue: recrue
      ? {
          weeks: [
            {
              weekStart: mondayOf(today()),
              pour: 2,
              contre: 3,
              blanc: 1,
              veto: index === 15 ? { by: member(1), motif: 'A parlé de la planque à un inconnu.' } : null,
            },
            { weekStart: addDays(mondayOf(today()), -7), pour: 4, contre: 1, blanc: 0, veto: null },
          ],
        }
      : null,
    phone: PHONES[index] ?? null,
    vehicles: VEHICLES[index] ?? [],
  }
}

// ---------------------------------------------------------------------------
// Configuration

const config = (): ConfigStatus => ({
  modules: [
    {
      key: 'presences',
      label: 'Présences',
      status: 'ok',
      checks: [
        { label: 'Salon des présences', ok: true, detail: '#presences', fix: null },
        {
          label: 'Rôle membre',
          ok: permissionRoles.memberRoleId !== null,
          detail: null,
          fix: '/config permission role_membre',
        },
      ],
    },
    {
      key: 'cotisations',
      label: 'Cotisations',
      status: objective.value ? 'ok' : 'incomplete',
      checks: [
        { label: 'Salon des cotisations', ok: true, detail: '#cotisations', fix: null },
        {
          label: 'Objectif hebdomadaire',
          ok: objective.value !== null,
          detail: objective.value ? null : 'Aucun objectif : pas de rappel le dimanche.',
          fix: null,
        },
      ],
    },
    {
      key: 'coffres',
      label: 'Coffres',
      status: 'incomplete',
      checks: [
        { label: 'Salons suivis', ok: true, detail: '3 salons', fix: null },
        {
          label: 'Webhook du salon 1290000000000000012',
          ok: false,
          detail: 'Aucun message du webhook reçu pour l’instant.',
          fix: '/config coffres salon',
        },
        { label: 'Alertes', ok: coffreAlerts.value.channelId !== null, detail: null, fix: null },
      ],
    },
    {
      key: 'recrues',
      label: 'Recrues',
      status: 'incomplete',
      checks: [
        { label: 'Salon d’essai', ok: true, detail: '#recrues', fix: null },
        { label: 'Rôle recrue', ok: false, detail: 'Rôle supprimé sur Discord.', fix: '/config recrues role' },
      ],
    },
    {
      key: 'transactions',
      label: 'Transactions',
      status: 'ok',
      checks: [{ label: 'Salon des transactions', ok: true, detail: '#transactions', fix: null }],
    },
    {
      key: 'contacts',
      label: 'Contacts',
      status: 'ok',
      checks: [{ label: 'Répertoire publié', ok: true, detail: '#repertoire', fix: null }],
    },
    {
      key: 'vehicules',
      label: 'Véhicules',
      status: 'off',
      checks: [
        { label: 'Salon des véhicules', ok: false, detail: 'Module non utilisé.', fix: '/config vehicules salon' },
      ],
    },
  ],
})

// ---------------------------------------------------------------------------
// Routeur

type Handler = (match: string[], params: URLSearchParams, body: unknown) => unknown

const routes: [string, RegExp, Handler][] = []
const route = (method: string, pattern: string, handler: Handler) => {
  routes.push([method, new RegExp(`^${pattern.replaceAll(/:[a-zA-Z]+/gu, '([^/]+)')}$`, 'u'), handler])
}

const G = '/api/guilds/:g'
const at = (match: string[], index: number) => decodeURIComponent(match[index] ?? '')

route('GET', '/api/me', () => me())
route('GET', `${G}/stats`, (m) => (requireGuild(at(m, 1), 'site.stats'), stats()))
route('GET', `${G}/members`, (m, p) => {
  requireGuild(at(m, 1))
  const q = (p.get('q') ?? '').toLowerCase()
  return MEMBERS.filter((ref) => ref.name.toLowerCase().includes(q)).slice(0, 25)
})
route('GET', `${G}/members/:id`, (m) => {
  const guild = requireGuild(at(m, 1))
  const id = at(m, 2)
  if (id !== ME.id && !guild.perms.includes('membre.fiche')) throw new HttpError(403, 'Fiche réservée aux gradés.')
  return profile(id)
})
route('GET', `${G}/transactions`, (m, p) => (requireGuild(at(m, 1), 'transaction.historique'), transactions(p)))
route('GET', `${G}/cotisations`, (m, p) => (requireGuild(at(m, 1), 'cotisation.stats'), cotisations(p)))
route('GET', `${G}/cotisations/export.csv`, (m, p) => (requireGuild(at(m, 1), 'cotisation.stats'), cotisationsCsv(p)))
route('POST', `${G}/cotisations`, (m, _p, body) => {
  const guild = requireGuild(at(m, 1), 'cotisation.ajouter')
  const input = body as NewCotisation
  if (input.memberId && input.memberId !== ME.id && !guild.perms.includes('cotisation.gerer')) {
    throw new HttpError(403, 'Déclarer pour un autre membre demande la permission cotisation.gerer.')
  }
  return addCotisation(input)
})
route('DELETE', `${G}/cotisations/:id`, (m) => {
  requireGuild(at(m, 1), 'cotisation.ajouter')
  const index = COTISATIONS.findIndex((entry) => entry.id === at(m, 2))
  if (index === -1) throw new HttpError(404, 'Cotisation introuvable.')
  COTISATIONS.splice(index, 1)
})
route('GET', `${G}/cotisations/objective`, (m) => (requireGuild(at(m, 1), 'cotisation.stats'), objective.value))
route('PUT', `${G}/cotisations/objective`, (m, _p, body) => {
  requireGuild(at(m, 1), 'admin')
  const input = body as Objective
  if (!Number.isInteger(input.amount) || input.amount <= 0) throw new HttpError(422, 'Le montant doit être positif.')
  objective.value = input
  return input
})
route('GET', `${G}/coffres/meta`, (m) => (requireGuild(at(m, 1), 'coffre.voir'), coffreMeta()))
route('GET', `${G}/coffres/stock`, (m, p) => (requireGuild(at(m, 1), 'coffre.voir'), coffreStock(p)))
route('GET', `${G}/coffres/movements`, (m, p) => {
  requireGuild(at(m, 1), 'coffre.voir')
  return paginate(filteredMovements(p), Number(p.get('page') ?? 1), 30)
})
route('GET', `${G}/coffres/players`, (m, p) => (requireGuild(at(m, 1), 'coffre.voir'), coffrePlayers(p)))
route('GET', `${G}/coffres/series`, (m, p) => (requireGuild(at(m, 1), 'coffre.voir'), coffreSeries(p)))
route('GET', `${G}/coffres/export.csv`, (m, p) => {
  requireGuild(at(m, 1), 'coffre.voir')
  const lines = filteredMovements(p).map((move) =>
    [move.at, move.kind, move.propertyName ?? move.propertyId, move.itemLabel, move.amount, move.player.name].join(';'),
  )
  return ['Date;Type;Propriété;Objet;Quantité;Personnage', ...lines].join('\n')
})
route('PUT', `${G}/coffres/properties/:id`, (m, _p, body) => {
  requireGuild(at(m, 1), 'coffre.gerer')
  const property = PROPERTIES.find((item) => item.id === at(m, 2))
  if (!property) throw new HttpError(404, 'Propriété introuvable.')
  const name = String((body as { name?: unknown }).name ?? '').trim()
  if (name.length > 60) throw new HttpError(422, 'Le nom dépasse 60 caractères.')
  property.name = name || null
  return property
})
route('PUT', `${G}/coffres/players/:id`, (m, _p, body) => {
  requireGuild(at(m, 1), 'coffre.gerer')
  const memberId = String((body as { memberId?: unknown }).memberId ?? '')
  requireMemberRole(memberById(memberId))
  LINKS.set(at(m, 2), memberId)
  return { gameId: at(m, 2), memberId }
})
route('DELETE', `${G}/coffres/players/:id`, (m) => {
  requireGuild(at(m, 1), 'coffre.gerer')
  if (!LINKS.delete(at(m, 2))) throw new HttpError(404, 'Ce personnage n’est relié à aucun membre.')
})
route('PUT', `${G}/coffres/inventory`, (m, _p, body) => {
  requireGuild(at(m, 1), 'coffre.gerer')
  return coffreInventory(body as CoffreInventory)
})
route('GET', `${G}/coffres/alerts`, (m) => (requireGuild(at(m, 1), 'coffre.voir'), coffreAlerts.value))
route('PUT', `${G}/coffres/alerts`, (m, _p, body) => {
  requireGuild(at(m, 1), 'admin')
  const input = body as CoffreAlerts
  if (input.thresholds.some((rule) => !rule.item || rule.min < 0)) {
    throw new HttpError(422, 'Chaque seuil demande un objet et une quantité positive.')
  }
  coffreAlerts.value = input
  return input
})
route('GET', `${G}/contacts`, (m, p) => {
  const guild = requireGuild(at(m, 1), 'contact.voir')
  return contactsList(p, guild.perms.includes('contact.notes'))
})
route('GET', `${G}/contacts/:id`, (m) => {
  const guild = requireGuild(at(m, 1), 'contact.voir')
  const canNotes = guild.perms.includes('contact.notes')
  const contact = findContact(at(m, 2))
  const notes = NOTES.get(contact.id) ?? []
  return { contact: { ...contact, notesCount: canNotes ? notes.length : null }, notes: canNotes ? notes : [] }
})
route('POST', `${G}/contacts`, (m, _p, body) => {
  requireGuild(at(m, 1), 'contact.ajouter')
  const input = body as ContactInput
  validateContact(input, false)
  const now = new Date().toISOString()
  const contact: Contact = {
    id: nextId(),
    name: input.name.trim(),
    kind: input.kind,
    relation: input.relation,
    phone: input.phone?.trim() || null,
    description: input.description?.trim() || null,
    createdBy: ME,
    createdAt: now,
    updatedAt: now,
    notesCount: 0,
  }
  CONTACTS.push(contact)
  NOTES.set(contact.id, [])
  return contact
})
route('PATCH', `${G}/contacts/:id`, (m, _p, body) => {
  requireGuild(at(m, 1), 'contact.modifier')
  const contact = findContact(at(m, 2))
  const input = body as Partial<ContactInput>
  validateContact(input, true)
  if (input.name !== undefined) contact.name = input.name.trim()
  if (input.kind !== undefined) contact.kind = input.kind
  if (input.relation !== undefined) contact.relation = input.relation
  if (input.phone !== undefined) contact.phone = input.phone?.trim() || null
  if (input.description !== undefined) contact.description = input.description?.trim() || null
  contact.updatedAt = new Date().toISOString()
  return contact
})
route('DELETE', `${G}/contacts/:id`, (m) => {
  requireGuild(at(m, 1), 'contact.modifier')
  const index = CONTACTS.findIndex((item) => item.id === at(m, 2))
  if (index === -1) throw new HttpError(404, 'Contact introuvable.')
  CONTACTS.splice(index, 1)
})
route('POST', `${G}/contacts/:id/notes`, (m, _p, body) => {
  requireGuild(at(m, 1), 'contact.notes')
  const contact = findContact(at(m, 2))
  const text = String((body as { body?: unknown }).body ?? '').trim()
  if (!text) throw new HttpError(422, 'La note est vide.')
  if (text.length > 1000) throw new HttpError(422, 'La note dépasse 1000 caractères.')
  const note: ContactNote = { id: nextId(), author: ME, body: text, createdAt: new Date().toISOString() }
  NOTES.set(contact.id, [note, ...(NOTES.get(contact.id) ?? [])])
  contact.notesCount = (contact.notesCount ?? 0) + 1
  return note
})
route('GET', `${G}/journal`, (m, p): JournalResponse => {
  requireGuild(at(m, 1), 'site.journal')
  const action = p.get('action')
  const list = JOURNAL.filter((entry) => !action || entry.action === action)
  return { actions: JOURNAL_ACTIONS, list: paginate(list, Number(p.get('page') ?? 1), 20) }
})
route('GET', `${G}/permissions`, (m) => (requireGuild(at(m, 1), 'admin'), permissionsResponse()))
route('PUT', `${G}/permissions`, (m, _p, body) => {
  requireGuild(at(m, 1), 'admin')
  const input = body as PermissionsUpdate
  if (input.memberRoleId !== undefined) permissionRoles.memberRoleId = input.memberRoleId
  for (const [key, level] of Object.entries(input.levels ?? {}) as [Perm, Level | null][]) {
    if (!ACTIONS.some((action) => action.key === key)) throw new HttpError(422, `Action inconnue : ${key}`)
    if (level === null) permissionLevels.delete(key)
    else permissionLevels.set(key, level)
  }
  return permissionsResponse()
})
route('GET', `${G}/config`, (m) => (requireGuild(at(m, 1), 'admin'), config()))
route('GET', '/api/tickets', (_m, p): TicketsResponse => {
  if (scenario() !== 'admin') throw new HttpError(403, 'Réservé au staff des tickets.')
  const q = (p.get('q') ?? '').toLowerCase().replace(/^#/u, '')
  const status = (p.get('status') ?? '').toUpperCase()
  const list = TICKETS.filter(
    (ticket) =>
      (!q || ticket.user.name.toLowerCase().includes(q) || String(ticket.number) === q || ticket.user.id === q) &&
      (!status || ticket.status === status),
  )
  return paginate(list, Number(p.get('page') ?? 1), 25)
})
route('GET', '/api/tickets/staff', () => {
  if (scenario() !== 'admin') throw new HttpError(403, 'Réservé au staff des tickets.')
  return ticketStats()
})
route('GET', '/api/logs/:id', (m): TicketLog => {
  const ticket = TICKETS.find((item) => item.id === at(m, 1)) ?? TICKETS[6]
  if (!ticket) throw new HttpError(404, 'Ticket introuvable.')
  const entries = ticketLogEntries(ticket.openedAt)
  return {
    number: ticket.number,
    user: { id: '106391128718245888', name: 'Elodie' },
    status: ticket.status,
    openedAt: ticket.openedAt,
    counts: {
      fromUser: entries.filter((entry) => entry.kind === 'user').length,
      toUser: entries.filter((entry) => entry.kind === 'staff').length,
      internal: entries.filter((entry) => entry.kind === 'internal').length,
    },
    rawUrl: `/logs/${at(m, 1)}?raw=1`,
    entries,
  }
})

// Accueil et santé.
route('GET', `${G}/overview`, (m): GuildOverview => {
  const guild = requireGuild(at(m, 1))
  const can = (perm: Perm) => guild.perms.includes(perm) && guild.id !== GUILD_B_ID
  const low = coffreStock(new URLSearchParams()).properties.flatMap((property) =>
    property.items
      .filter((item) => item.low)
      .map((item) => {
        const rule = coffreAlerts.value.thresholds.find((threshold) => threshold.item === item.item)
        return {
          propertyId: property.id,
          propertyName: property.name,
          item: item.item,
          label: item.label,
          quantity: item.quantity,
          min: rule?.min ?? 0,
        }
      }),
  )
  const week = objectiveProgress()
  const mine = objectiveProgress(ME.id)[0] ?? null
  const open = TRANSACTIONS.filter((item) => !['LIVRE_PAYE', 'ANNULEE', 'CLOTUREE'].includes(item.status))
  const concerned = week.filter((row) => !row.exempt)
  const [last] = JOURNAL
  return {
    lowStock: can('coffre.voir') ? { count: low.length, items: low.slice(0, 3) } : null,
    objective: can('cotisation.ajouter')
      ? {
          progress: mine,
          reached: can('cotisation.stats') && mine ? concerned.filter((row) => row.reached).length : null,
          tracked: can('cotisation.stats') && mine ? concerned.length : null,
        }
      : null,
    presence: can('presence.stats')
      ? {
          next: {
            id: 'presence-prochaine',
            title: PRESENCE_TITLES[0] ?? 'Présence',
            date: new Date(NOW + 26 * 3_600_000).toISOString(),
            myStatus: 'NON_REPONDU',
            responses: 9,
          },
        }
      : null,
    absences: can('presence.stats')
      ? {
          count: stats().absencesCurrent.length,
          members: stats().absencesCurrent.map((absence) => absence.member),
        }
      : null,
    recrues: can('recrue.suivi')
      ? { count: stats().recruits.length, missingVotes: can('recrue.avis') ? 1 : null }
      : null,
    transactions: can('transaction.utiliser')
      ? { count: open.length, amount: open.reduce((sum, item) => sum + item.prix, 0) }
      : null,
    tickets:
      scenario() === 'admin' && guild.id !== GUILD_B_ID
        ? { open: TICKETS.filter((t) => t.status === 'OPEN').length }
        : null,
    journal: can('site.journal') ? { last: last ? { at: last.at, actor: last.actor, label: last.label } : null } : null,
  }
})
const daysBefore = (days: number) => new Date(NOW - days * 86_400_000).toISOString()

route('GET', `${G}/health`, (m, p): GuildHealth => {
  requireGuild(at(m, 1), 'admin')
  const silentDays = Number(p.get('jours') ?? 7) || 7
  const last = daysBefore
  return {
    version: '2.0.0',
    commit: '82a4b2b0c1d2',
    startedAt: last(3.2),
    uptimeSeconds: Math.round(3.2 * 86_400),
    databaseBytes: 18_874_368,
    messageContent: true,
    shards: [{ id: 0, connected: true, latencyMs: 74, seenAt: last(0.0003) }],
    silentDays,
    channels: [
      {
        channelId: '1290000000000000010',
        name: 'coffre-planque',
        accessible: true,
        webhooks: 2,
        lastMovementAt: last(0.2),
        lastCatchUpAt: last(3.2),
        lastCatchUpOk: true,
        silent: false,
      },
      {
        channelId: '1290000000000000012',
        name: 'coffre-garage',
        accessible: true,
        webhooks: 1,
        lastMovementAt: last(9),
        lastCatchUpAt: last(3.2),
        lastCatchUpOk: true,
        silent: silentDays < 9,
      },
      {
        channelId: '1290000000000000014',
        name: null,
        accessible: false,
        webhooks: 0,
        lastMovementAt: null,
        lastCatchUpAt: last(3.2),
        lastCatchUpOk: false,
        silent: true,
      },
    ],
    global: true,
    errors: [
      {
        at: last(0.05),
        level: 'WARN',
        target: 'bot_discord::modules::coffres::ingest',
        message: 'coffres : rattrapage impossible : Response error: status code 403',
      },
      {
        at: last(1.4),
        level: 'ERROR',
        target: 'bot_discord::web::api',
        message: "erreur de l'API : database is locked",
      },
    ],
  }
})

// Présences, absences, recrues, annuaire, écritures des transactions : groupe.ts.
registerGroupeRoutes({
  route,
  requireGuild,
  fail: (status, message) => new HttpError(status, message),
  memberById,
  requireMemberRole,
  paginate,
})

const delay = (ms: number) =>
  // oxlint-disable-next-line promise/avoid-new -- seule façon d'attendre un minuteur
  new Promise<void>((resolve) => {
    globalThis.setTimeout(resolve, ms)
  })

export const mockFetch = async (input: string, init: RequestInit): Promise<Response> => {
  const url = new URL(input, globalThis.location.origin)
  const method = (init.method ?? 'GET').toUpperCase()
  const body: unknown = typeof init.body === 'string' ? JSON.parse(init.body) : undefined
  // Latence simulée.
  await delay(120 + Math.random() * 250)
  if (method !== 'GET' && new Headers(init.headers).get('X-Requested-With') !== 'fetch') {
    return Response.json({ error: 'En-tête X-Requested-With manquant.' }, { status: 400 })
  }
  for (const [routeMethod, pattern, handler] of routes) {
    const match = pattern.exec(url.pathname)
    if (!match || routeMethod !== method) continue
    try {
      const result = handler([...match], url.searchParams, body)
      if (result === undefined) return new Response(null, { status: 204 })
      if (typeof result === 'string') return new Response(result, { headers: { 'Content-Type': 'text/csv' } })
      return Response.json(result)
    } catch (error) {
      if (error instanceof HttpError) {
        const payload =
          error.status === 401 ? { error: error.message, loginUrl: '/?mock=admin' } : { error: error.message }
        return Response.json(payload, { status: error.status })
      }
      throw error
    }
  }
  return Response.json({ error: 'Route inconnue de la maquette.' }, { status: 404 })
}

// Les liens d'export CSV (téléchargements directs) ne passent pas par fetch : ils restent
// sans effet en maquette. Le serveur du bot les sert en production.
