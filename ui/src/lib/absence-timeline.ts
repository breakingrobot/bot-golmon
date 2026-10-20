// Frise des absences : une ligne par membre, une colonne par jour de Paris (~5 semaines, aujourd'hui marqué).
// Fonctions pures : jours de la fenêtre, barres découpées à la fenêtre, navigation au clavier.

import type { AbsenceEntry, MemberRef } from '@/api/types'

/** Cinq semaines : la précédente, la semaine en cours et les trois suivantes. */
export const TIMELINE_DAYS = 35

export type AbsenceState = 'current' | 'upcoming' | 'past'

export type TimelineBar = {
  absence: AbsenceEntry
  state: AbsenceState
  /** Première colonne (0 = premier jour de la fenêtre). */
  start: number
  /** Nombre de jours couverts dans la fenêtre (1 au moins). */
  span: number
  /** L'absence commence avant la fenêtre, ou finit après. */
  clippedStart: boolean
  clippedEnd: boolean
}

export type TimelineRow = { member: MemberRef; bars: TimelineBar[] }

const PARIS_DAY = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Paris' })

/** Jour de Paris (`YYYY-MM-DD`) d'un instant ISO. */
export const parisDayOf = (iso: string): string => PARIS_DAY.format(new Date(iso))

/** Instant présent en ISO (comparé aux dates de l'API). */
export const nowIso = (): string => new Date().toISOString()

/** `YYYY-MM-DD` décalé de `days` jours (calendrier, sans fuseau). */
export const shiftDay = (day: string, days: number): string => {
  const date = new Date(`${day}T12:00:00Z`)
  date.setUTCDate(date.getUTCDate() + days)
  return date.toISOString().slice(0, 10)
}

/** Lundi de la semaine d'un jour. */
export const mondayOf = (day: string): string => {
  const weekday = (new Date(`${day}T12:00:00Z`).getUTCDay() + 6) % 7
  return shiftDay(day, -weekday)
}

/** Jours de la fenêtre : du lundi de la semaine précédente, `count` jours. */
export const timelineDays = (today: string, count = TIMELINE_DAYS): string[] => {
  const first = shiftDay(mondayOf(today), -7)
  return Array.from({ length: count }, (_, index) => shiftDay(first, index))
}

/** Fin effective : fin anticipée ou annulation, sinon fin prévue. */
export const effectiveEnd = (absence: AbsenceEntry): string =>
  absence.endedAt && absence.endedAt < absence.to ? absence.endedAt : absence.to

export const absenceState = (absence: AbsenceEntry, now: string): AbsenceState => {
  if (effectiveEnd(absence) <= now) return 'past'
  return absence.from <= now ? 'current' : 'upcoming'
}

/** Annulée avant d'avoir commencé : rien à dessiner. */
const cancelledBeforeStart = (absence: AbsenceEntry): boolean =>
  absence.endedAt !== null && absence.endedAt <= absence.from

/**
 * Lignes de la frise : un membre par ligne (absence la plus proche d'abord), barres limitées à la fenêtre.
 * Les absences entièrement hors de la fenêtre sont écartées.
 */
export const timelineRows = (absences: AbsenceEntry[], days: string[], now: string): TimelineRow[] => {
  const [first] = days
  const last = days.at(-1)
  if (!first || !last) return []
  const rows = new Map<string, TimelineRow>()
  const sorted = absences
    .filter((absence) => !cancelledBeforeStart(absence))
    .toSorted((a, b) => a.from.localeCompare(b.from))
  for (const absence of sorted) {
    const from = parisDayOf(absence.from)
    const to = parisDayOf(effectiveEnd(absence))
    if (to < first || from > last) continue
    const start = from < first ? 0 : days.indexOf(from)
    const end = to > last ? days.length - 1 : days.indexOf(to)
    if (start < 0 || end < start) continue
    const row = rows.get(absence.member.id) ?? { member: absence.member, bars: [] }
    row.bars.push({
      absence,
      state: absenceState(absence, now),
      start,
      span: end - start + 1,
      clippedStart: from < first,
      clippedEnd: to > last,
    })
    rows.set(absence.member.id, row)
  }
  return [...rows.values()]
}

export type TimelineFocus = { row: number; bar: number }

/**
 * Barre suivante au clavier : ← → dans la ligne, ↑ ↓ vers la barre la plus proche de la ligne voisine,
 * Début / Fin aux extrémités de la ligne. `null` : touche sans effet.
 */
export const moveFocus = (rows: TimelineRow[], focus: TimelineFocus, key: string): TimelineFocus | null => {
  const row = rows[focus.row]
  const bar = row?.bars[focus.bar]
  if (!row || !bar) return null
  switch (key) {
    case 'ArrowRight': {
      return focus.bar + 1 < row.bars.length ? { row: focus.row, bar: focus.bar + 1 } : null
    }
    case 'ArrowLeft': {
      return focus.bar > 0 ? { row: focus.row, bar: focus.bar - 1 } : null
    }
    case 'Home': {
      return { row: focus.row, bar: 0 }
    }
    case 'End': {
      return { row: focus.row, bar: row.bars.length - 1 }
    }
    case 'ArrowDown':
    case 'ArrowUp': {
      const target = focus.row + (key === 'ArrowDown' ? 1 : -1)
      const next = rows[target]
      if (!next || next.bars.length === 0) return null
      // Barre de la ligne voisine la plus proche de celle qui a le focus.
      let best = 0
      next.bars.forEach((candidate, index) => {
        const current = next.bars[best]
        if (current && Math.abs(candidate.start - bar.start) < Math.abs(current.start - bar.start)) best = index
      })
      return { row: target, bar: best }
    }
    default: {
      return null
    }
  }
}
