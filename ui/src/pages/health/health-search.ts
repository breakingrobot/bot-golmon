// Paramètres d'adresse de la page « Santé » : seuil de l'alerte « webhook silencieux ».

export const SILENT_PERIODS = ['3', '7', '14', '30'] as const

export type SilentPeriod = (typeof SILENT_PERIODS)[number]

export type HealthSearch = { jours?: SilentPeriod }
