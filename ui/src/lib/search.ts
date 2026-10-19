// Lecture des paramètres d'adresse (filtres) : chaînes et entiers, valeurs vides ignorées.

export function searchString(value: unknown): string | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) return String(value)
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim()
  return trimmed ? trimmed.slice(0, 200) : undefined
}

export function searchPage(value: unknown): number | undefined {
  const number = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : Number.NaN
  return Number.isInteger(number) && number > 1 ? number : undefined
}

export function searchDay(value: unknown): string | undefined {
  const text = searchString(value)
  return text && /^\d{4}-\d{2}-\d{2}$/.test(text) ? text : undefined
}

export function searchOneOf<T extends string>(value: unknown, allowed: readonly T[]): T | undefined {
  const text = searchString(value)
  return allowed.find((item) => item === text)
}

/** Objet sans les clés `undefined` (compatible `exactOptionalPropertyTypes`). */
export function compact<T extends Record<string, unknown>>(value: T): { [K in keyof T]?: Exclude<T[K], undefined> } {
  const result: Record<string, unknown> = {}
  for (const [key, item] of Object.entries(value)) {
    if (item !== undefined) result[key] = item
  }
  return result as { [K in keyof T]?: Exclude<T[K], undefined> }
}

/** Modification partielle de paramètres d'adresse : `undefined` retire le paramètre. */
export type SearchPatch<T> = { [K in keyof T]?: T[K] | undefined }

/** Valeur sentinelle des listes déroulantes Radix (qui refusent la chaîne vide). */
export const ALL = 'tous'

export function fromSelect(value: string): string | undefined {
  return value === ALL ? undefined : value
}
