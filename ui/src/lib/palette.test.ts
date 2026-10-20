import { describe, expect, it } from 'vitest'

import type { CoffreMeta, Contact, GuildAccess } from '@/api/types'
import { allCommands, filterCommands } from '@/lib/palette'
import type { PaletteSources } from '@/lib/palette'
import { TEST_GUILD, TEST_ME } from '@/test/render'

const contact: Contact = {
  id: 'c1',
  name: 'Église du Port',
  kind: 'GROUPE',
  relation: 'NEUTRE',
  phone: '555-0101',
  description: null,
  createdBy: { id: '1', name: 'Léo', avatarUrl: null },
  createdAt: '2026-10-01T10:00:00Z',
  updatedAt: '2026-10-01T10:00:00Z',
  notesCount: null,
}

const coffres: CoffreMeta = {
  properties: [{ id: '8740', name: 'Planque du port' }],
  items: [{ item: 'repair_kit', label: 'Kit de réparation' }],
  links: [],
  channels: [],
}

const sources = (guild: GuildAccess): PaletteSources => ({
  me: { ...TEST_ME, guilds: [guild, { ...guild, id: '2', name: 'Autre serveur' }] },
  guild,
  members: [{ id: '42', name: 'Élodie', avatarUrl: null }],
  contacts: [contact],
  coffres,
})

const labels = (guild: GuildAccess, query: string) =>
  filterCommands(allCommands(sources(guild)), query).map((section) => [
    section.group,
    section.commands.map((command) => command.label),
  ])

describe('palette de commandes', () => {
  it('sans recherche : pages, actions et serveurs seulement', () => {
    const groups = labels(TEST_GUILD, '').map(([group]) => group)
    expect(groups).toStrictEqual(['pages', 'actions', 'serveurs'])
  })

  it('cherche sans accents, dans les sources ouvertes au compte', () => {
    const guild: GuildAccess = {
      ...TEST_GUILD,
      perms: ['contact.voir', 'coffre.voir', 'membre.fiche'],
    }
    expect(labels(guild, 'elodie')).toStrictEqual([['membres', ['Élodie']]])
    expect(labels(guild, 'eglise')).toStrictEqual([['contacts', ['Église du Port']]])
    expect(labels(guild, 'reparation')).toStrictEqual([['coffres', ['Kit de réparation']]])
    expect(labels(guild, 'planque')).toStrictEqual([['coffres', ['Planque du port']]])
  })

  it('n’ouvre que ce que les droits permettent', () => {
    const member: GuildAccess = { ...TEST_GUILD, isAdmin: false, perms: ['cotisation.ajouter'] }
    expect(labels(member, 'elodie')).toStrictEqual([])
    expect(labels(member, 'cotisation')).toStrictEqual([['actions', ['Déclarer une cotisation']]])
    expect(labels(member, 'permissions')).toStrictEqual([])
    expect(labels(member, 'theme')).toStrictEqual([['actions', ['Basculer le thème']]])
  })
})
