// Maquette des pages Présences, Absences, Recrues, Annuaire et des écritures des transactions
// (branchée par server.ts, mêmes outils : droits du scénario, erreurs HTTP, pagination).

import type {
  AbsenceEntry,
  GuildAccess,
  MemberRef,
  NewAbsence,
  NewPresence,
  NewPresenceSchedule,
  NewTransaction,
  Page,
  Perm,
  PhoneEntry,
  PresenceDetail,
  PresenceSchedule,
  PresenceStats,
  PresenceSummary,
  Recrue,
  RecrueVoteInput,
  Transaction,
  TransactionItemInput,
  TransactionStatus,
  VehicleEntry,
  VehicleInput,
} from '@/api/types'

import {
  addDays,
  daysAgo,
  int,
  ME,
  MEMBERS,
  member,
  mondayOf,
  nextId,
  NOW,
  parisDay,
  PHONES,
  pick,
  PRESENCE_TITLES,
  random,
  role,
  ROLES,
  STATUS_LABELS,
  TRANSACTIONS,
  VEHICLES,
} from './fixtures'

type Handler = (match: string[], params: URLSearchParams, body: unknown) => unknown

export type MockTools = {
  route: (method: string, pattern: string, handler: Handler) => void
  requireGuild: (guildId: string, perm?: Perm | 'admin') => GuildAccess
  fail: (status: number, message: string) => Error
  memberById: (id: string) => MemberRef
  requireMemberRole: (person: MemberRef) => void
  paginate: <T>(items: T[], page: number, pageSize?: number) => Page<T>
}

const DAY = 86_400_000
const G = '/api/guilds/:g'
const at = (match: string[], index: number) => decodeURIComponent(match[index] ?? '')
const WEEKDAYS = ['lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi', 'dimanche']
const MEMBRES = MEMBERS.slice(0, 14)

// ---------------------------------------------------------------------------
// Données

type MockPresence = { summary: PresenceSummary; answers: Map<string, 'PRESENT' | 'ABSENT' | 'RETARD'> }

const presence = (title: string, date: string, closed: boolean): MockPresence => {
  const answers = new Map<string, 'PRESENT' | 'ABSENT' | 'RETARD'>()
  for (const person of MEMBRES) {
    const roll = random()
    if (closed ? roll < 0.9 : roll < 0.5) {
      answers.set(person.id, roll < 0.6 ? 'PRESENT' : roll < 0.75 ? 'RETARD' : 'ABSENT')
    }
  }
  const count = (status: string) => [...answers.values()].filter((value) => value === status).length
  return {
    summary: {
      id: nextId(),
      title,
      description: closed ? null : 'Tenue correcte, rendez-vous au garage.',
      date,
      closedAt: closed ? date : null,
      createdBy: member(int(0, 4)),
      present: count('PRESENT'),
      absent: count('ABSENT'),
      retard: count('RETARD'),
      noAnswer: MEMBRES.length - answers.size,
    },
    answers,
  }
}

const PRESENCES: MockPresence[] = [
  presence('Run Elysian', new Date(NOW + 2 * DAY).toISOString(), false),
  presence('Soirée au port', new Date(NOW + 6 * 3_600_000).toISOString(), false),
  ...Array.from({ length: 26 }, (_, index) => presence(pick(PRESENCE_TITLES), daysAgo(index * 3 + 1), true)),
]

const SCHEDULES: PresenceSchedule[] = [
  {
    id: nextId(),
    weekday: 3,
    weekdayLabel: 'mercredi',
    time: '21:00',
    title: 'Réunion du mercredi',
    description: null,
    leadHours: 24,
    nextEvent: new Date(NOW + 3 * DAY).toISOString(),
    createdBy: ME,
  },
]

const absence = (index: number, fromDays: number, toDays: number, motif: string): AbsenceEntry => ({
  id: nextId(),
  member: member(index),
  motif,
  from: `${addDays(parisDay(NOW), fromDays - 1)}T22:00:00Z`,
  to: `${addDays(parisDay(NOW), toDays)}T21:59:00Z`,
  endedAt: null,
  createdBy: member(index === 7 ? 1 : index),
})

const ABSENCES: AbsenceEntry[] = [
  absence(5, -3, 4, 'Vacances en famille'),
  absence(8, -1, 1, 'Travail de nuit'),
  absence(0, 6, 12, 'Déménagement'),
  absence(7, 15, 20, 'Examens'),
  { ...absence(10, -20, -12, 'Voyage'), endedAt: daysAgo(12) },
]

const RECRUES: Recrue[] = [14, 15, 16].map((index, position) => ({
  entryId: nextId(),
  member: member(index),
  since: daysAgo(position * 9 + 4),
  weeks: position + 1,
  pour: 0,
  contre: 0,
  blanc: 0,
  veto: position === 2 ? { by: member(1), motif: 'Comportement en jeu à revoir.' } : null,
  myVote: null,
  votes: [
    { voter: member(1), choice: 'POUR', motif: 'Motivé, toujours présent.' },
    { voter: member(2), choice: position === 1 ? 'CONTRE' : 'POUR', motif: 'Bon contact avec le groupe.' },
    { voter: member(3), choice: 'BLANC', motif: 'Pas encore assez vu.' },
  ],
}))

const recount = (recrue: Recrue) => {
  const votes = recrue.votes ?? []
  recrue.pour = votes.filter((vote) => vote.choice === 'POUR').length
  recrue.contre = votes.filter((vote) => vote.choice === 'CONTRE').length
  recrue.blanc = votes.filter((vote) => vote.choice === 'BLANC').length
}
for (const recrue of RECRUES) recount(recrue)

const VEHICLE_LIST: VehicleEntry[] = [
  ...Object.entries(VEHICLES).flatMap(([index, list]) =>
    list.map((item) => ({
      plate: item.plate.replaceAll(' ', ''),
      model: item.model,
      ownerMember: member(Number(index)),
      ownerRole: null,
    })),
  ),
  { plate: '88MEC01', model: 'Flatbed', ownerMember: null, ownerRole: role('Mécano') },
]

const PHONE_LIST: PhoneEntry[] = Object.entries(PHONES).map(([index, number]) => ({
  member: member(Number(index)),
  number,
}))

const matches = (q: string, fields: (string | null | undefined)[]) => {
  const text = q.toLowerCase().replaceAll(/[\s-]/gu, '')
  return !text || fields.some((field) => (field ?? '').toLowerCase().replaceAll(/[\s-]/gu, '').includes(text))
}

// ---------------------------------------------------------------------------
// Calculs

const presenceStats = (days: number | null): PresenceStats => {
  const since = days === null ? 0 : NOW - days * DAY
  const closed = PRESENCES.filter((item) => item.summary.closedAt && Date.parse(item.summary.date) >= since)
  const members = MEMBRES.map((person) => {
    const answers = closed.map((item) => item.answers.get(person.id))
    const count = (status: string) => answers.filter((answer) => answer === status).length
    const expected = closed.length
    const present = count('PRESENT')
    return {
      member: person,
      expected,
      present,
      retard: count('RETARD'),
      absent: count('ABSENT'),
      noAnswer: answers.filter((answer) => answer === undefined).length,
      rate: expected > 0 ? Math.round((present * 100) / expected) : null,
    }
  }).toSorted((a, b) => (b.rate ?? -1) - (a.rate ?? -1))
  const present = members.reduce((sum, row) => sum + row.present, 0)
  const expected = members.reduce((sum, row) => sum + row.expected, 0)
  const weekCount = days === null ? 12 : Math.max(1, Math.ceil(days / 7))
  const lastMonday = mondayOf(parisDay(NOW))
  const weekly = Array.from({ length: weekCount }, (_, index) => {
    const weekStart = addDays(lastMonday, (index - weekCount + 1) * 7)
    const inWeek = closed.filter((item) => mondayOf(parisDay(item.summary.date)) === weekStart)
    const total = inWeek.length * MEMBRES.length
    const yes = inWeek.reduce((sum, item) => sum + item.summary.present, 0)
    return { weekStart, rate: total > 0 ? Math.round((yes * 100) / total) : null, presences: inWeek.length }
  })
  return {
    presences: closed.length,
    rate: expected > 0 ? Math.round((present * 100) / expected) : null,
    threshold: 50,
    members,
    weekly,
  }
}

const detail = (item: MockPresence): PresenceDetail => {
  const of = (status: string) =>
    [...item.answers.entries()]
      .filter(([, value]) => value === status)
      .map(([id]) => ({
        member: MEMBRES.find((person) => person.id === id) ?? ME,
        retardMinutes: status === 'RETARD' ? 15 : null,
        changed: false,
      }))
  return {
    presence: item.summary,
    present: of('PRESENT'),
    absent: of('ABSENT'),
    retard: of('RETARD'),
    noAnswer: MEMBRES.filter((person) => !item.answers.has(person.id)),
    changes: [],
    roster: MEMBRES.length,
  }
}

const setStatus = (item: Transaction, status: TransactionStatus) => {
  item.status = status
  item.statusLabel = STATUS_LABELS[status]
  return item
}

// ---------------------------------------------------------------------------
// Routes

export const registerGroupeRoutes = ({
  route,
  requireGuild,
  fail,
  memberById,
  requireMemberRole,
  paginate,
}: MockTools) => {
  const requireGroup = (guildId: string) => {
    const guild = requireGuild(guildId)
    if (!guild.inGroup && !guild.isAdmin) throw fail(403, 'Réservé aux membres du groupe.')
    return guild
  }
  const findPresence = (id: string) => {
    const found = PRESENCES.find((item) => item.summary.id === id)
    if (!found) throw fail(404, 'Présence introuvable.')
    return found
  }

  route('GET', `${G}/presences`, (m, p) => {
    requireGuild(at(m, 1), 'presence.stats')
    const state = p.get('etat')
    const list = PRESENCES.map((item) => item.summary).filter((item) => {
      if (state === 'closes') return item.closedAt !== null
      if (state === 'ouvertes') return item.closedAt === null
      if (state === 'a_venir') return item.closedAt === null && Date.parse(item.date) > Date.now()
      return true
    })
    return paginate(list, Number(p.get('page') ?? 1), 20)
  })
  route('GET', `${G}/presences/stats`, (m, p) => {
    requireGuild(at(m, 1), 'presence.stats')
    const raw = p.get('jours') ?? '30'
    return presenceStats(raw === 'tout' ? null : Number(raw) || 30)
  })
  route('GET', `${G}/presences/export.csv`, (m) => (requireGuild(at(m, 1), 'presence.stats'), 'Membre;Taux'))
  route('GET', `${G}/presences/plannings`, (m) => (requireGuild(at(m, 1), 'presence.gerer'), SCHEDULES))
  route('POST', `${G}/presences/plannings`, (m, _p, body) => {
    requireGuild(at(m, 1), 'presence.gerer')
    const input = body as NewPresenceSchedule
    if (input.weekday < 1 || input.weekday > 7) throw fail(422, 'Jour invalide.')
    const schedule: PresenceSchedule = {
      id: nextId(),
      weekday: input.weekday,
      weekdayLabel: WEEKDAYS[input.weekday - 1] ?? '',
      time: input.time,
      title: input.title ?? 'Présence',
      description: input.description ?? null,
      leadHours: input.leadHours ?? 24,
      nextEvent: new Date(Date.now() + 4 * DAY).toISOString(),
      createdBy: ME,
    }
    SCHEDULES.push(schedule)
    return schedule
  })
  route('DELETE', `${G}/presences/plannings/:id`, (m) => {
    requireGuild(at(m, 1), 'presence.gerer')
    const index = SCHEDULES.findIndex((item) => item.id === at(m, 2))
    if (index === -1) throw fail(404, 'Présence planifiée introuvable.')
    SCHEDULES.splice(index, 1)
  })
  route('GET', `${G}/presences/:id`, (m) => (requireGuild(at(m, 1), 'presence.stats'), detail(findPresence(at(m, 2)))))
  route('POST', `${G}/presences`, (m, _p, body) => {
    requireGuild(at(m, 1), 'presence.creer')
    const input = body as NewPresence
    const day = input.date ?? parisDay(Date.now())
    const created = presence(input.title ?? 'Présence', new Date(`${day}T${input.time}:00`).toISOString(), false)
    created.answers.clear()
    Object.assign(created.summary, { present: 0, absent: 0, retard: 0, noAnswer: MEMBRES.length, createdBy: ME })
    created.summary.description = input.description ?? null
    PRESENCES.unshift(created)
    return created.summary
  })
  route('POST', `${G}/presences/:id/close`, (m) => {
    requireGuild(at(m, 1), 'presence.creer')
    const item = findPresence(at(m, 2))
    if (item.summary.closedAt) throw fail(422, 'Cette présence est déjà clôturée.')
    item.summary.closedAt = new Date().toISOString()
    return detail(item)
  })

  route('GET', `${G}/absences`, (m) => {
    requireGroup(at(m, 1))
    const now = new Date().toISOString()
    return {
      current: ABSENCES.filter((item) => !item.endedAt && item.from <= now && item.to > now),
      upcoming: ABSENCES.filter((item) => !item.endedAt && item.from > now),
      recent: ABSENCES.filter((item) => item.endedAt || item.to <= now),
    }
  })
  route('POST', `${G}/absences`, (m, _p, body) => {
    const guild = requireGroup(at(m, 1))
    const input = body as NewAbsence
    if (input.memberId && input.memberId !== ME.id && !guild.perms.includes('absence.autrui')) {
      throw fail(403, '« Absence d’un autre membre, panneau des absences » est réservé aux gradés et leads.')
    }
    const person = input.memberId ? memberById(input.memberId) : ME
    requireMemberRole(person)
    const from = input.from ?? parisDay(Date.now())
    const created: AbsenceEntry = {
      id: nextId(),
      member: person,
      motif: input.motif,
      from: `${addDays(from, -1)}T22:00:00Z`,
      to: `${input.to}T21:59:00Z`,
      endedAt: null,
      createdBy: ME,
    }
    ABSENCES.push(created)
    return { absence: created, affected: 0, roleWarning: null }
  })
  route('POST', `${G}/absences/:id/end`, (m) => {
    requireGroup(at(m, 1))
    const found = ABSENCES.find((item) => item.id === at(m, 2) && !item.endedAt)
    if (!found) throw fail(404, 'Absence introuvable, ou déjà terminée.')
    const cancelled = Date.parse(found.from) > Date.now()
    found.endedAt = new Date().toISOString()
    return { cancelled, roleWarning: null }
  })

  const findRecrue = (id: string) => {
    const found = RECRUES.find((item) => item.entryId === id)
    if (!found) throw fail(404, 'Cette recrue n’est plus suivie.')
    return found
  }
  route('GET', `${G}/recrues`, (m) => {
    const guild = requireGuild(at(m, 1))
    if (!['recrue.suivi', 'recrue.avis', 'recrue.veto'].some((perm) => guild.perms.includes(perm as Perm))) {
      throw fail(403, '« Donner un avis sur une recrue » est réservé aux gradés et leads.')
    }
    const detailed = guild.perms.includes('recrue.suivi')
    return {
      configured: true,
      weekStart: mondayOf(parisDay(Date.now())),
      recruits: RECRUES.map((recrue) => ({ ...recrue, votes: detailed ? recrue.votes : null })),
    }
  })
  route('GET', `${G}/recrues/:id/history`, (m) => {
    requireGuild(at(m, 1), 'recrue.suivi')
    const found = RECRUES.find((item) => item.member.id === at(m, 2))
    if (!found) return []
    const monday = mondayOf(parisDay(Date.now()))
    return Array.from({ length: found.weeks }, (_, index) => ({
      weekStart: addDays(monday, -7 * index),
      pour: found.pour,
      contre: found.contre,
      blanc: found.blanc,
      veto: index === 0 ? found.veto : null,
      votes: found.votes ?? [],
    }))
  })
  route('PUT', `${G}/recrues/:id/vote`, (m, _p, body) => {
    requireGuild(at(m, 1), 'recrue.avis')
    const recrue = findRecrue(at(m, 2))
    const input = body as RecrueVoteInput
    if (!input.motif.trim()) throw fail(422, 'Le motif est obligatoire.')
    recrue.votes = [...(recrue.votes ?? []).filter((vote) => vote.voter.id !== ME.id), { voter: ME, ...input }]
    recrue.myVote = input
    recount(recrue)
    return input
  })
  route('PUT', `${G}/recrues/:id/veto`, (m, _p, body) => {
    requireGuild(at(m, 1), 'recrue.veto')
    const recrue = findRecrue(at(m, 2))
    if (recrue.veto) throw fail(422, `Cette recrue est déjà sous véto de ${recrue.veto.by.name}. Lève-le d'abord.`)
    recrue.veto = { by: ME, motif: String((body as { motif?: unknown }).motif ?? '') }
    return recrue.veto
  })
  route('DELETE', `${G}/recrues/:id/veto`, (m) => {
    requireGuild(at(m, 1), 'recrue.veto')
    const recrue = findRecrue(at(m, 2))
    if (!recrue.veto) throw fail(422, 'Aucun véto actif sur cette recrue.')
    recrue.veto = null
  })

  route('GET', `${G}/vehicules`, (m, p) => {
    requireGuild(at(m, 1), 'vehicule.utiliser')
    const q = p.get('q') ?? ''
    return {
      items: VEHICLE_LIST.filter((item) =>
        matches(q, [item.plate, item.model, item.ownerMember?.name, item.ownerRole?.name]),
      ),
      roles: ROLES.filter((item) => item.name !== 'Bot'),
    }
  })
  route('POST', `${G}/vehicules`, (m, _p, body) => {
    requireGuild(at(m, 1), 'vehicule.utiliser')
    const input = body as VehicleInput
    if (VEHICLE_LIST.some((item) => item.plate === input.plate)) {
      throw fail(422, `La plaque ${input.plate} est déjà enregistrée.`)
    }
    const ownerRole = input.ownerIsRole ? (ROLES.find((item) => item.id === input.ownerId) ?? null) : null
    const ownerMember = input.ownerIsRole ? null : memberById(input.ownerId)
    if (ownerMember) requireMemberRole(ownerMember)
    const created: VehicleEntry = { plate: input.plate, model: input.model ?? 'Kamacho', ownerMember, ownerRole }
    VEHICLE_LIST.push(created)
    return created
  })
  route('DELETE', `${G}/vehicules/:plate`, (m) => {
    requireGuild(at(m, 1), 'vehicule.utiliser')
    const index = VEHICLE_LIST.findIndex((item) => item.plate === at(m, 2))
    if (index === -1) throw fail(404, 'Aucun véhicule enregistré avec cette plaque.')
    VEHICLE_LIST.splice(index, 1)
  })
  route('GET', `${G}/numeros`, (m, p) => {
    requireGroup(at(m, 1))
    const q = p.get('q') ?? ''
    return PHONE_LIST.filter((item) => matches(q, [item.member.name, item.number]))
  })
  route('PUT', `${G}/numeros/:user`, (m, _p, body) => {
    const guild = requireGroup(at(m, 1))
    const person = memberById(at(m, 2))
    if (person.id !== ME.id && !guild.perms.includes('numero.ajouter_autrui')) {
      throw fail(403, '« Enregistrer le numéro d’un autre membre » est réservé à « Gérer le serveur ».')
    }
    requireMemberRole(person)
    const digits = String((body as { number?: unknown }).number ?? '').replaceAll(/\D/gu, '')
    if (!/^555\d{4,6}$/u.test(digits)) throw fail(422, 'Format invalide.')
    const number = `555-${digits.slice(3)}`
    if (PHONE_LIST.some((item) => item.number === number && item.member.id !== person.id)) {
      throw fail(422, 'Ce numéro est déjà attribué à un autre membre.')
    }
    const existing = PHONE_LIST.find((item) => item.member.id === person.id)
    if (existing) existing.number = number
    else PHONE_LIST.push({ member: person, number })
    return { member: person, number }
  })
  route('DELETE', `${G}/numeros/:user`, (m) => {
    const guild = requireGroup(at(m, 1))
    if (at(m, 2) !== ME.id && !guild.perms.includes('numero.supprimer_autrui')) {
      throw fail(403, '« Supprimer le numéro d’un autre membre » est réservé aux gradés et leads.')
    }
    const index = PHONE_LIST.findIndex((item) => item.member.id === at(m, 2))
    if (index === -1) throw fail(404, 'Aucun numéro enregistré pour ce membre.')
    PHONE_LIST.splice(index, 1)
  })

  const openTransaction = (guildId: string, id: string): Transaction => {
    requireGuild(guildId, 'transaction.utiliser')
    const found = TRANSACTIONS.find((item) => item.id === id)
    if (!found) throw fail(404, 'Transaction introuvable.')
    if (found.status === 'CLOTUREE') throw fail(422, 'Cette transaction est clôturée et ne peut plus être modifiée.')
    return found
  }
  route('POST', `${G}/transactions`, (m, _p, body) => {
    requireGuild(at(m, 1), 'transaction.utiliser')
    const input = body as NewTransaction
    const created: Transaction = {
      id: nextId(),
      createdAt: new Date().toISOString(),
      groupe: input.groupe,
      type: input.type,
      status: 'PREPARATION',
      statusLabel: STATUS_LABELS.PREPARATION,
      prix: input.prix,
      creator: ME,
      items: (input.items ?? []).map((item) => ({
        nom: item.nom,
        quantite: item.quantite ?? 1,
        prixUnitaire: item.prixUnitaire ?? null,
      })),
    }
    TRANSACTIONS.unshift(created)
    return created
  })
  route('PUT', `${G}/transactions/:id/status`, (m, _p, body) =>
    setStatus(openTransaction(at(m, 1), at(m, 2)), (body as { status: TransactionStatus }).status),
  )
  route('POST', `${G}/transactions/:id/items`, (m, _p, body) => {
    const item = openTransaction(at(m, 1), at(m, 2))
    const input = body as TransactionItemInput
    item.items.push({ nom: input.nom, quantite: input.quantite ?? 1, prixUnitaire: input.prixUnitaire ?? null })
    return item
  })
  route('POST', `${G}/transactions/:id/cancel`, (m) => setStatus(openTransaction(at(m, 1), at(m, 2)), 'ANNULEE'))
}
