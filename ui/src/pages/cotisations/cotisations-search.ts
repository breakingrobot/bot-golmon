export const PERIODS = [
  { value: '7', label: '7 jours' },
  { value: '30', label: '30 jours' },
  { value: '90', label: '90 jours' },
  { value: '365', label: '1 an' },
  { value: 'tout', label: 'Tout' },
] as const

export type Period = (typeof PERIODS)[number]['value']

export type CotisationsSearch = { jours?: Period; membre?: string; page?: number }
