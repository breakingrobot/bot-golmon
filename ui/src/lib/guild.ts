// Pages d'un serveur et droits nécessaires (le serveur vérifie, le front masque seulement).

import type { LucideIcon } from 'lucide-react'
import {
  ArrowLeftRight,
  BarChart3,
  BookUser,
  CalendarCheck,
  CalendarOff,
  Coins,
  Contact,
  ScrollText,
  Settings2,
  ShieldCheck,
  UserPlus,
  UserRound,
  Users,
  Vault,
} from 'lucide-react'

import type { GuildAccess, Me, Perm } from '@/api/types'

export type GuildPageKey =
  | 'stats'
  | 'transactions'
  | 'cotisations'
  | 'coffres'
  | 'contacts'
  | 'presences'
  | 'absences'
  | 'recrues'
  | 'annuaire'
  | 'journal'
  | 'membres'
  | 'fiche'
  | 'permissions'
  | 'configuration'

/** Routes d'un serveur sans autre paramètre que `guildId` (la fiche est traitée à part). */
export type GuildPagePath =
  | '/serveurs/$guildId'
  | '/serveurs/$guildId/transactions'
  | '/serveurs/$guildId/cotisations'
  | '/serveurs/$guildId/coffres'
  | '/serveurs/$guildId/contacts'
  | '/serveurs/$guildId/presences'
  | '/serveurs/$guildId/absences'
  | '/serveurs/$guildId/recrues'
  | '/serveurs/$guildId/annuaire'
  | '/serveurs/$guildId/journal'
  | '/serveurs/$guildId/membres'
  | '/serveurs/$guildId/permissions'
  | '/serveurs/$guildId/configuration'

export type GuildPage = {
  key: GuildPageKey
  label: string
  /** `null` : fiche du membre connecté (`/serveurs/$guildId/membres/$userId`). */
  to: GuildPagePath | null
  icon: LucideIcon
  section: 'serveur' | 'reglages'
}

export const GUILD_PAGES: GuildPage[] = [
  { key: 'stats', label: 'Statistiques', to: '/serveurs/$guildId', icon: BarChart3, section: 'serveur' },
  {
    key: 'transactions',
    label: 'Transactions',
    to: '/serveurs/$guildId/transactions',
    icon: ArrowLeftRight,
    section: 'serveur',
  },
  { key: 'cotisations', label: 'Cotisations', to: '/serveurs/$guildId/cotisations', icon: Coins, section: 'serveur' },
  { key: 'coffres', label: 'Coffres', to: '/serveurs/$guildId/coffres', icon: Vault, section: 'serveur' },
  { key: 'contacts', label: 'Contacts', to: '/serveurs/$guildId/contacts', icon: BookUser, section: 'serveur' },
  {
    key: 'presences',
    label: 'Présences',
    to: '/serveurs/$guildId/presences',
    icon: CalendarCheck,
    section: 'serveur',
  },
  { key: 'absences', label: 'Absences', to: '/serveurs/$guildId/absences', icon: CalendarOff, section: 'serveur' },
  { key: 'recrues', label: 'Recrues', to: '/serveurs/$guildId/recrues', icon: UserPlus, section: 'serveur' },
  { key: 'annuaire', label: 'Annuaire', to: '/serveurs/$guildId/annuaire', icon: Contact, section: 'serveur' },
  { key: 'journal', label: 'Journal du staff', to: '/serveurs/$guildId/journal', icon: ScrollText, section: 'serveur' },
  { key: 'membres', label: 'Membres', to: '/serveurs/$guildId/membres', icon: Users, section: 'serveur' },
  { key: 'fiche', label: 'Ma fiche', to: null, icon: UserRound, section: 'serveur' },
  {
    key: 'permissions',
    label: 'Permissions',
    to: '/serveurs/$guildId/permissions',
    icon: ShieldCheck,
    section: 'reglages',
  },
  {
    key: 'configuration',
    label: 'Configuration',
    to: '/serveurs/$guildId/configuration',
    icon: Settings2,
    section: 'reglages',
  },
]

/** Actions strictes des recrues : une seule suffit pour ouvrir la page. */
export const RECRUE_PERMS: Perm[] = ['recrue.suivi', 'recrue.avis', 'recrue.veto']

export const hasPerm = (guild: GuildAccess | undefined, perm: Perm): boolean => guild?.perms.includes(perm) ?? false

export const canSeePage = (guild: GuildAccess, key: GuildPageKey): boolean => {
  switch (key) {
    case 'stats': {
      return hasPerm(guild, 'site.stats')
    }
    case 'transactions': {
      return hasPerm(guild, 'transaction.historique')
    }
    case 'cotisations': {
      return hasPerm(guild, 'cotisation.stats')
    }
    case 'coffres': {
      return hasPerm(guild, 'coffre.voir')
    }
    case 'contacts': {
      return hasPerm(guild, 'contact.voir')
    }
    case 'presences': {
      return hasPerm(guild, 'presence.stats')
    }
    case 'absences': {
      // Ouvert au groupe, comme `/absence list` ; « Gérer le serveur » pour administrer.
      return guild.inGroup || guild.isAdmin
    }
    case 'recrues': {
      return RECRUE_PERMS.some((perm) => hasPerm(guild, perm))
    }
    case 'annuaire': {
      // Véhicules (`vehicule.utiliser`) ou numéros (membres du groupe).
      return hasPerm(guild, 'vehicule.utiliser') || guild.inGroup || guild.isAdmin
    }
    case 'journal': {
      return hasPerm(guild, 'site.journal')
    }
    case 'membres': {
      return hasPerm(guild, 'membre.fiche')
    }
    case 'fiche': {
      // Chacun peut voir sa propre fiche.
      return true
    }
    case 'permissions':
    case 'configuration': {
      return guild.isAdmin
    }
  }
}

export const visiblePages = (guild: GuildAccess): GuildPage[] =>
  GUILD_PAGES.filter((page) => canSeePage(guild, page.key))

export const findGuild = (me: Me | undefined, guildId: string): GuildAccess | undefined =>
  me?.guilds.find((guild) => guild.id === guildId)
