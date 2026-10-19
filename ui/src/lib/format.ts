// Mise en forme française : montants « 12 500 $ », dates dans le fuseau du lecteur.

const LOCALE = 'fr-FR'

const integer = new Intl.NumberFormat(LOCALE, { maximumFractionDigits: 0 })
const decimal = new Intl.NumberFormat(LOCALE, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
const compact = new Intl.NumberFormat(LOCALE, { notation: 'compact', maximumFractionDigits: 1 })

/** Espaces fines insécables du format français remplacées par des espaces insécables (meilleur rendu des polices). */
function spaces(text: string): string {
  return text.replace(/ /g, ' ')
}

export function formatNumber(value: number): string {
  return spaces(integer.format(value))
}

/** « 12 500 $ » ; décimales seulement si présentes (prix des transactions). */
export function formatMoney(value: number, withDecimals = false): string {
  const format = withDecimals && !Number.isInteger(value) ? decimal : integer
  return `${spaces(format.format(value))} $`
}

/** Montant signé (« +1 200 $ », « −300 $ »). */
export function formatSignedMoney(value: number): string {
  const sign = value > 0 ? '+' : value < 0 ? '−' : ''
  return `${sign}${formatMoney(Math.abs(value))}`
}

/** Axe des graphiques : « 12,5 k $ ». */
export function formatCompactMoney(value: number): string {
  return `${spaces(compact.format(value))} $`
}

export function formatPercent(value: number): string {
  return `${formatNumber(value)} %`
}

const dateTime = new Intl.DateTimeFormat(LOCALE, { dateStyle: 'medium', timeStyle: 'short' })
const dateOnly = new Intl.DateTimeFormat(LOCALE, { dateStyle: 'medium' })
const longDate = new Intl.DateTimeFormat(LOCALE, { dateStyle: 'long' })
const timeOnly = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit' })
const dayUtc = new Intl.DateTimeFormat(LOCALE, { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' })
const dayShortUtc = new Intl.DateTimeFormat(LOCALE, { day: 'numeric', month: 'short', timeZone: 'UTC' })
const monthUtc = new Intl.DateTimeFormat(LOCALE, { month: 'short', year: '2-digit', timeZone: 'UTC' })
const monthLongUtc = new Intl.DateTimeFormat(LOCALE, { month: 'long', year: 'numeric', timeZone: 'UTC' })

function parse(iso: string): Date | null {
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? null : date
}

/** Instant ISO (UTC) affiché dans le fuseau du lecteur. */
export function formatDateTime(iso: string): string {
  const date = parse(iso)
  return date ? dateTime.format(date) : iso
}

export function formatDate(iso: string): string {
  const date = parse(iso)
  return date ? dateOnly.format(date) : iso
}

export function formatLongDate(iso: string): string {
  const date = parse(iso)
  return date ? longDate.format(date) : iso
}

export function formatTime(iso: string): string {
  const date = parse(iso)
  return date ? timeOnly.format(date) : iso
}

/** Jour calendaire `YYYY-MM-DD` (jour de Paris) : affiché tel quel, sans conversion de fuseau. */
function dayDate(day: string): Date | null {
  // Un instant ISO complet est ramené au jour de Paris.
  if (day.length > 10) {
    const instant = parse(day)
    if (!instant) return null
    day = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Paris' }).format(instant)
  }
  return parse(`${day.slice(0, 10)}T12:00:00Z`)
}

export function formatDay(day: string): string {
  const date = dayDate(day)
  return date ? dayUtc.format(date) : day
}

export function formatShortDay(day: string): string {
  const date = dayDate(day)
  return date ? dayShortUtc.format(date) : day
}

/** Mois `YYYY-MM`. */
export function formatMonth(month: string, long = false): string {
  const date = dayDate(`${month}-01`)
  return date ? (long ? monthLongUtc : monthUtc).format(date) : month
}

export function formatWeek(weekStart: string): string {
  return `Sem. du ${formatShortDay(weekStart)}`
}

const relative = new Intl.RelativeTimeFormat(LOCALE, { numeric: 'auto' })
const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 31_536_000],
  ['month', 2_592_000],
  ['week', 604_800],
  ['day', 86_400],
  ['hour', 3600],
  ['minute', 60],
  ['second', 1],
]

export function formatRelative(iso: string, now = Date.now()): string {
  const date = parse(iso)
  if (!date) return iso
  const seconds = (date.getTime() - now) / 1000
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size || unit === 'second') return relative.format(Math.round(seconds / size), unit)
  }
  return iso
}

/** Aujourd'hui en `YYYY-MM-DD`, jour de Paris (comme le serveur). */
export function todayParis(): string {
  return new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Paris' }).format(new Date())
}

export function plural(count: number, singular: string, pluralForm = `${singular}s`): string {
  return `${formatNumber(count)} ${count > 1 ? pluralForm : singular}`
}
