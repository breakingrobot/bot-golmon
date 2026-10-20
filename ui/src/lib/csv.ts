// Export CSV côté client, au même format que les exports du bot (src/web/*.rs) : séparateur `;`,
// UTF-8 avec BOM (Excel), fins de ligne CRLF, nombres à virgule décimale, dates `JJ/MM/AAAA HH:MM`
// (heure de Paris), cellules qui commencent par `=`, `+`, `-` ou `@` préfixées d'une apostrophe.

export type CsvValue = string | number | null | undefined

/** Une colonne : titre et valeur d'une ligne. */
export type CsvColumn<T> = { header: string; value: (row: T) => CsvValue }

/** Neutralise les formules : Excel exécute une cellule qui commence par `=`, `+`, `-` ou `@`. */
const neutralize = (text: string): string => (/^[=+\-@\t\r]/u.test(text) ? `'${text}` : text)

/** Guillemets (doublés à l'intérieur) dès que le champ contient `;`, `"` ou un saut de ligne (RFC 4180). */
const quote = (text: string): string => (/[;"\n\r]/u.test(text) ? `"${text.replaceAll('"', '""')}"` : text)

/** Cellule : texte neutralisé puis échappé, nombre à virgule décimale sans séparateur de milliers. */
export const csvCell = (value: CsvValue): string => {
  if (value === null || value === undefined) return ''
  if (typeof value === 'number') return Number.isFinite(value) ? String(value).replace('.', ',') : ''
  return quote(neutralize(value))
}

/** Document complet : BOM, ligne de titres, une ligne par élément, CRLF. */
export const toCsv = <T>(rows: readonly T[], columns: readonly CsvColumn<T>[]): string => {
  const lines = [
    columns.map((column) => csvCell(column.header)).join(';'),
    ...rows.map((row) => columns.map((column) => csvCell(column.value(row))).join(';')),
  ]
  return `﻿${lines.join('\r\n')}\r\n`
}

const parisParts = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'Europe/Paris',
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
})

/** Instant ISO → `JJ/MM/AAAA HH:MM`, heure de Paris (comme les exports du bot). */
export const csvDateTime = (iso: string): string => {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parisParts.formatToParts(date).find((item) => item.type === type)?.value ?? ''
  return `${part('day')}/${part('month')}/${part('year')} ${part('hour')}:${part('minute')}`
}

/** Jour `YYYY-MM-DD` → `JJ/MM/AAAA`. */
export const csvDay = (day: string): string => {
  const groups = /^(?<year>\d{4})-(?<month>\d{2})-(?<date>\d{2})/u.exec(day)?.groups
  return groups ? `${groups['date']}/${groups['month']}/${groups['year']}` : day
}

/** Texte d'un HTML rendu par le serveur (détails du journal), pour une cellule. */
export const htmlToText = (html: string): string =>
  (new DOMParser().parseFromString(html, 'text/html').body.textContent ?? '').replaceAll(/\s+/gu, ' ').trim()

/** Téléchargement d'un texte CSV (lien temporaire, rien ne part sur le réseau). */
export const downloadCsv = (filename: string, text: string): void => {
  const url = URL.createObjectURL(new Blob([text], { type: 'text/csv;charset=utf-8' }))
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.append(link)
  link.click()
  link.remove()
  globalThis.setTimeout(() => URL.revokeObjectURL(url), 0)
}
