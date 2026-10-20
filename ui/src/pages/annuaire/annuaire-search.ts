export const TABS = [
  { value: 'vehicules', label: 'Véhicules' },
  { value: 'numeros', label: 'Numéros' },
] as const

export type Tab = (typeof TABS)[number]['value']

export type AnnuaireSearch = { onglet?: Tab; q?: string }
