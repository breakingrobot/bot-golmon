export const PERIODS = [
  { value: '1', label: '24 h' },
  { value: '7', label: '7 jours' },
  { value: '30', label: '30 jours' },
  { value: '90', label: '90 jours' },
  { value: 'tout', label: 'Tout' },
] as const

export type Period = (typeof PERIODS)[number]['value']

export type CoffresSearch = {
  propriete?: string
  objet?: string
  joueur?: string
  type?: 'deposit' | 'withdraw'
  jours?: Period
  page?: number
}
