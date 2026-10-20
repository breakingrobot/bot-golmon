// Palette de commandes : entrées proposées selon les droits, filtrées sans accents, regroupées.
// Les membres viennent de l'API (recherche du serveur), le reste est filtré ici.

import type { CoffreMeta, Contact, GuildAccess, Me, MemberRef } from '@/api/types'
import { hasPerm, visiblePages } from '@/lib/guild'
import type { GuildPage } from '@/lib/guild'
import { matchesQuery } from '@/lib/text'

export type CommandGroup = 'pages' | 'actions' | 'membres' | 'contacts' | 'coffres' | 'serveurs'

export const GROUP_LABELS: Record<CommandGroup, string> = {
  pages: 'Pages',
  actions: 'Actions',
  membres: 'Membres',
  contacts: 'Contacts',
  coffres: 'Coffres',
  serveurs: 'Changer de serveur',
}

const GROUP_ORDER: CommandGroup[] = ['pages', 'actions', 'membres', 'contacts', 'coffres', 'serveurs']

/** Entrées montrées par groupe. */
export const GROUP_LIMIT = 6

export type CommandTarget =
  | { kind: 'page'; guildId: string; page: GuildPage }
  | { kind: 'tickets'; to: '/tickets' | '/tickets/staff' }
  | { kind: 'guild'; guildId: string }
  | { kind: 'member'; guildId: string; memberId: string }
  | { kind: 'contact'; guildId: string; contactId: string }
  | { kind: 'item'; guildId: string; item: string }
  | { kind: 'property'; guildId: string; propertyId: string }
  | { kind: 'cotisation' }
  | { kind: 'theme' }

export type Command = {
  id: string
  group: CommandGroup
  label: string
  /** Précision affichée à droite (type, relation, identifiant…). */
  hint?: string
  /** Mots cherchés en plus du libellé. */
  keywords?: string
  target: CommandTarget
}

export type PaletteSources = {
  me: Me | undefined
  guild: GuildAccess | undefined
  members: MemberRef[]
  contacts: Contact[]
  coffres: CoffreMeta | undefined
}

const RELATIONS: Record<Contact['relation'], string> = {
  ALLIE: 'Allié',
  NEUTRE: 'Neutre',
  HOSTILE: 'Hostile',
}

/** Toutes les entrées possibles (avant filtrage), selon les droits du serveur courant. */
export const allCommands = ({ me, guild, members, contacts, coffres }: PaletteSources): Command[] => {
  const commands: Command[] = []
  if (guild) {
    for (const page of visiblePages(guild)) {
      commands.push({
        id: `page:${page.key}`,
        group: 'pages',
        label: page.label,
        ...(page.section === 'reglages' ? { hint: 'Réglages' } : {}),
        target: { kind: 'page', guildId: guild.id, page },
      })
    }
    if (hasPerm(guild, 'cotisation.ajouter')) {
      commands.push({
        id: 'action:cotisation',
        group: 'actions',
        label: 'Déclarer une cotisation',
        keywords: 'ajouter argent propre sale',
        target: { kind: 'cotisation' },
      })
    }
    if (hasPerm(guild, 'membre.fiche')) {
      for (const member of members) {
        commands.push({
          id: `membre:${member.id}`,
          group: 'membres',
          label: member.name,
          keywords: member.id,
          target: { kind: 'member', guildId: guild.id, memberId: member.id },
        })
      }
    }
    if (hasPerm(guild, 'contact.voir')) {
      for (const contact of contacts) {
        commands.push({
          id: `contact:${contact.id}`,
          group: 'contacts',
          label: contact.name,
          hint: RELATIONS[contact.relation],
          keywords: [contact.phone, contact.description].filter(Boolean).join(' '),
          target: { kind: 'contact', guildId: guild.id, contactId: contact.id },
        })
      }
    }
    if (coffres && hasPerm(guild, 'coffre.voir')) {
      for (const item of coffres.items) {
        commands.push({
          id: `objet:${item.item}`,
          group: 'coffres',
          label: item.label,
          hint: 'Objet',
          keywords: item.item,
          target: { kind: 'item', guildId: guild.id, item: item.item },
        })
      }
      for (const property of coffres.properties) {
        commands.push({
          id: `propriete:${property.id}`,
          group: 'coffres',
          label: property.name ?? `Propriété ${property.id}`,
          hint: 'Propriété',
          keywords: `${property.id} propriete`,
          target: { kind: 'property', guildId: guild.id, propertyId: property.id },
        })
      }
    }
  }
  if (me?.tickets) {
    commands.push(
      {
        id: 'tickets',
        group: 'pages',
        label: 'Tickets',
        target: { kind: 'tickets', to: '/tickets' },
      },
      {
        id: 'tickets:staff',
        group: 'pages',
        label: 'Statistiques du staff',
        keywords: 'tickets',
        target: { kind: 'tickets', to: '/tickets/staff' },
      },
    )
  }
  commands.push({
    id: 'action:theme',
    group: 'actions',
    label: 'Basculer le thème',
    keywords: 'clair sombre nuit jour',
    target: { kind: 'theme' },
  })
  for (const other of me?.guilds ?? []) {
    if (other.id === guild?.id) continue
    commands.push({
      id: `serveur:${other.id}`,
      group: 'serveurs',
      label: other.name,
      target: { kind: 'guild', guildId: other.id },
    })
  }
  return commands
}

export type CommandSection = { group: CommandGroup; label: string; commands: Command[] }

/** Groupes non vides, dans l'ordre d'affichage, 6 entrées au plus chacun. Sans recherche : pages, actions, serveurs. */
export const filterCommands = (commands: Command[], query: string): CommandSection[] => {
  const searching = query.trim() !== ''
  return GROUP_ORDER.filter((group) => searching || group === 'pages' || group === 'actions' || group === 'serveurs')
    .map((group) => ({
      group,
      label: GROUP_LABELS[group],
      commands: commands
        .filter((command) => command.group === group)
        .filter((command) => matchesQuery(query, command.label, command.hint, command.keywords))
        .slice(0, searching || group !== 'pages' ? GROUP_LIMIT : undefined),
    }))
    .filter((section) => section.commands.length > 0)
}
