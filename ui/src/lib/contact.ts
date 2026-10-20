import type { Contact, ContactKind, Relation } from '@/api/types'
import type { BadgeVariant } from '@/atoms/Badge'
import { csvDateTime } from '@/lib/csv'
import type { CsvColumn } from '@/lib/csv'

export const LIMITS = {
  name: 80,
  phone: 30,
  description: 500,
  note: 1000,
} as const satisfies Record<string, number>

export const KIND_LABELS: Record<ContactKind, string> = {
  GROUPE: 'Groupe',
  ENTREPRISE: 'Entreprise',
  PERSONNE: 'Personne',
}

export const RELATION_LABELS: Record<Relation, string> = {
  ALLIE: 'Allié',
  NEUTRE: 'Neutre',
  HOSTILE: 'Hostile',
}

export const RELATION_VARIANT: Record<Relation, BadgeVariant> = {
  ALLIE: 'success',
  NEUTRE: 'neutral',
  HOSTILE: 'danger',
}

export const RELATION_PLURALS: Record<Relation, string> = {
  ALLIE: 'Alliés',
  NEUTRE: 'Neutres',
  HOSTILE: 'Hostiles',
}

/** Colonnes de l'export CSV du répertoire. */
export const CONTACT_CSV: CsvColumn<Contact>[] = [
  { header: 'Nom', value: (contact) => contact.name },
  { header: 'Type', value: (contact) => KIND_LABELS[contact.kind] },
  { header: 'Relation', value: (contact) => RELATION_LABELS[contact.relation] },
  { header: 'Téléphone', value: (contact) => contact.phone },
  { header: 'Description', value: (contact) => contact.description },
  { header: 'Notes', value: (contact) => contact.notesCount },
  { header: 'Ajouté par', value: (contact) => contact.createdBy.name },
  { header: 'Ajouté le', value: (contact) => csvDateTime(contact.createdAt) },
  { header: 'Modifié le', value: (contact) => csvDateTime(contact.updatedAt) },
]
