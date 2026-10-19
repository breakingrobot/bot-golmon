import type { Level } from '@/api/types'

export const LEVELS: { value: Level; label: string; description: string }[] = [
  { value: 'membre', label: 'Membres du groupe', description: 'Rôle membre (obligatoire pour tout le reste)' },
  { value: 'grade', label: 'Gradés', description: 'Rôle gradé et au-dessus' },
  { value: 'lead', label: 'Lead', description: 'Rôle lead et « Gérer le serveur »' },
  { value: 'admin', label: 'Administrateurs', description: '« Gérer le serveur » seulement' },
]

export const LEVEL_LABEL = Object.fromEntries(LEVELS.map((level) => [level.value, level.label])) as Record<
  Level,
  string
>

export const NO_ROLE = 'aucun'
