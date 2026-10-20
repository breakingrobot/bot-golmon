// Inventaire des coffres : mêmes règles que le bot (`/coffre inventaire` et PUT …/coffres/inventory).

/** Quantité maximale d'un objet dans un inventaire. */
export const MAX_INVENTORY = 1_000_000

/** Objets au plus par inventaire. */
export const MAX_INVENTORY_ITEMS = 300

export type InventoryEntry = { item: string; quantity: number }

/** Identifiant d'objet du jeu (`repair_kit`) : minuscules, chiffres, `_`, `-`, `.` ; les espaces deviennent `_`. */
export const normalizeItem = (raw: string): string | null => {
  const item = raw.trim().toLowerCase().replaceAll(' ', '_')
  return item.length > 0 && item.length <= 64 && /^[a-z0-9_.-]+$/u.test(item) ? item : null
}

/** Numéro de propriété du jeu (`8740`). */
export const isPropertyId = (raw: string): boolean => /^\d{1,30}$/u.test(raw.trim())

/** Entier signé lu dans la saisie (espaces de milliers et `_` admis), `null` s'il est illisible. */
const readInteger = (raw: string): number | null => {
  const text = raw.replaceAll(/[\s_]/gu, '')
  return /^[+-]?\d{1,16}$/u.test(text) ? Number(text) : null
}

const inRange = (quantity: number) => quantity >= 0 && quantity <= MAX_INVENTORY

/** Quantité saisie : entier de 0 à 1 000 000, espaces de milliers admis ; `null` si illisible ou hors bornes. */
export const parseQuantity = (raw: string): number | null => {
  const quantity = readInteger(raw)
  return quantity !== null && inRange(quantity) ? quantity : null
}

/** Nom lisible d'un identifiant inconnu (`repair_kit` → « Repair kit »), comme le bot. */
export const fallbackItemLabel = (item: string): string => {
  const spaced = item.replaceAll(/[_-]/gu, ' ').trim()
  return spaced ? `${spaced.charAt(0).toUpperCase()}${spaced.slice(1)}` : item
}

/** `objet:quantité` ou `objet=quantité` ; à défaut, le dernier espace sépare (`acetone 2790`). */
const splitEntry = (part: string): [string, string] | null => {
  const separator = part.search(/[:=]/u)
  if (separator !== -1) return [part.slice(0, separator), part.slice(separator + 1)]
  const space = part.lastIndexOf(' ')
  return space === -1 ? null : [part.slice(0, space), part.slice(space + 1)]
}

export type ParsedList = { ok: true; entries: InventoryEntry[] } | { ok: false; error: string }

/**
 * Liste « objet:quantité » de la commande Discord : entrées séparées par `,`, `;` ou des retours à la ligne,
 * `objet:quantité`, `objet=quantité` ou, à défaut, `objet quantité` (dernier espace). Un objet répété garde
 * sa dernière quantité. Renvoie un message clair à la première entrée illisible.
 */
export const parseInventoryList = (raw: string): ParsedList => {
  const entries: InventoryEntry[] = []
  const parts = raw
    .split(/[,;\n]/u)
    .map((part) => part.trim())
    .filter(Boolean)
  for (const part of parts) {
    const split = splitEntry(part)
    if (!split) return { ok: false, error: `« ${part} » : écris objet:quantité, par exemple repair_kit:42.` }
    const [rawItem = '', rawQuantity = ''] = split
    const quantity = readInteger(rawQuantity)
    if (quantity === null) return { ok: false, error: `« ${part} » : quantité illisible.` }
    const item = normalizeItem(rawItem)
    if (!item) return { ok: false, error: `« ${part} » : nom d’objet invalide.` }
    if (!inRange(quantity)) return { ok: false, error: `« ${part} » : quantité entre 0 et ${MAX_INVENTORY}.` }
    const known = entries.find((entry) => entry.item === item)
    if (known) known.quantity = quantity
    else entries.push({ item, quantity })
  }
  if (entries.length === 0) return { ok: false, error: 'Indique au moins un objet, par exemple repair_kit:42.' }
  return { ok: true, entries }
}

export type InventoryDiff = { item: string; label: string; before: number; after: number; delta: number }

/**
 * Écarts entre le stock connu et les quantités comptées : seuls les objets qui changent, dans l'ordre de saisie
 * (un objet inconnu part de 0). C'est exactement ce que le bot enregistrera comme corrections.
 */
export const inventoryDiff = (
  known: readonly { item: string; label: string; quantity: number }[],
  counted: readonly InventoryEntry[],
  labelOf: (item: string) => string = fallbackItemLabel,
): InventoryDiff[] => {
  // Un objet saisi deux fois garde sa dernière quantité, à la place de la première.
  const last = new Map<string, number>()
  for (const { item, quantity } of counted) last.set(item, quantity)
  const diffs: InventoryDiff[] = []
  for (const [item, after] of last) {
    const current = known.find((entry) => entry.item === item)
    const before = current?.quantity ?? 0
    if (after === before) continue
    diffs.push({ item, label: current?.label ?? labelOf(item), before, after, delta: after - before })
  }
  return diffs
}
