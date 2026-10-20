export const TABS = [
  { value: 'liste', label: 'Présences' },
  { value: 'assiduite', label: 'Assiduité' },
  { value: 'plannings', label: 'Plannings' },
] as const

export type Tab = (typeof TABS)[number]['value']

export const STATES = [
  { value: 'a_venir', label: 'À venir' },
  { value: 'ouvertes', label: 'Ouvertes' },
  { value: 'closes', label: 'Closes' },
] as const

export type State = (typeof STATES)[number]['value']

export const PERIODS = [
  { value: '7', label: '7 jours' },
  { value: '30', label: '30 jours' },
  { value: '90', label: '90 jours' },
  { value: '365', label: '1 an' },
  { value: 'tout', label: 'Tout' },
] as const

export type Period = (typeof PERIODS)[number]['value']

export type PresencesSearch = {
  onglet?: Tab
  etat?: State
  du?: string
  au?: string
  page?: number
  /** Présence ouverte dans le panneau de détail. */
  presence?: string
  jours?: Period
}
