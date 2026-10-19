import type { ContactKind, Relation } from '@/api/types'

export const RELATIONS = ['ALLIE', 'NEUTRE', 'HOSTILE'] as const

export const KINDS = ['GROUPE', 'ENTREPRISE', 'PERSONNE'] as const

export type ContactsSearch = { relation?: Relation; type?: ContactKind; q?: string; contact?: string }
