export const VIEWS = [
  { value: 'frise', label: 'Frise' },
  { value: 'liste', label: 'Liste' },
] as const

export type View = (typeof VIEWS)[number]['value']

export type AbsencesSearch = { vue?: View }
