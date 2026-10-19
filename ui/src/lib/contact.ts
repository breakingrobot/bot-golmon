import type { ContactKind, Relation } from '@/api/types'
import type { BadgeVariant } from '@/atoms/Badge'

export const LIMITS = { name: 80, phone: 30, description: 500, note: 1000 } as const

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
