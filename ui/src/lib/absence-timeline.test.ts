import { describe, expect, it } from 'vitest'

import type { AbsenceEntry, MemberRef } from '@/api/types'
import {
  absenceState,
  mondayOf,
  moveFocus,
  parisDayOf,
  shiftDay,
  timelineDays,
  timelineRows,
  TIMELINE_DAYS,
} from '@/lib/absence-timeline'

const member = (id: string): MemberRef => ({ id, name: `Membre ${id}`, avatarUrl: null })

/** Absence en jours de Paris (début 00:00, fin 23:59), heure d'été (UTC+2) : avant le 25 octobre. */
const absence = (
  id: string,
  who: string,
  [from, to]: [string, string],
  endedAt: string | null = null,
): AbsenceEntry => ({
  id,
  member: member(who),
  motif: 'Vacances',
  from: `${shiftDay(from, -1)}T22:00:00Z`,
  to: `${to}T21:59:00Z`,
  endedAt,
  createdBy: member(who),
})

// Mardi 20 octobre 2026, midi à Paris.
const NOW = '2026-10-20T10:00:00Z'
const DAYS = timelineDays('2026-10-20')

describe('fenêtre de la frise', () => {
  it('trouve le lundi de la semaine', () => {
    expect(mondayOf('2026-10-20')).toBe('2026-10-19')
    expect(mondayOf('2026-10-25')).toBe('2026-10-19')
  })

  it('commence le lundi de la semaine précédente, sur cinq semaines', () => {
    expect(DAYS).toHaveLength(TIMELINE_DAYS)
    expect(DAYS[0]).toBe('2026-10-12')
    expect(DAYS.at(-1)).toBe('2026-11-15')
    // Changement d'heure du 25 octobre : les jours restent consécutifs.
    expect(DAYS.indexOf('2026-10-26') - DAYS.indexOf('2026-10-25')).toBe(1)
  })

  it('ramène un instant au jour de Paris', () => {
    expect(parisDayOf('2026-10-19T22:00:00Z')).toBe('2026-10-20')
    expect(parisDayOf('2026-10-20T21:59:00Z')).toBe('2026-10-20')
  })
})

describe('barres des absences', () => {
  it('place chaque absence dans sa colonne et distingue en cours, à venir et passée', () => {
    const rows = timelineRows(
      [
        absence('a', '1', ['2026-10-19', '2026-10-22']),
        absence('b', '2', ['2026-10-21', '2026-10-23']),
        absence('c', '1', ['2026-10-13', '2026-10-14']),
      ],
      DAYS,
      NOW,
    )
    expect(rows.map((row) => row.member.id)).toStrictEqual(['1', '2'])
    const [first, second] = rows
    expect(first?.bars.map((bar) => [bar.absence.id, bar.state, bar.start, bar.span])).toStrictEqual([
      ['c', 'past', 1, 2],
      ['a', 'current', 7, 4],
    ])
    expect(second?.bars[0]).toMatchObject({ state: 'upcoming', start: 9, span: 3 })
  })

  it('coupe les absences qui débordent de la fenêtre et écarte celles hors fenêtre', () => {
    const rows = timelineRows(
      [
        absence('long', '1', ['2026-09-01', '2026-12-31']),
        absence('avant', '2', ['2026-09-01', '2026-09-10']),
        absence('apres', '3', ['2026-12-01', '2026-12-10']),
      ],
      DAYS,
      NOW,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.bars[0]).toMatchObject({ start: 0, span: TIMELINE_DAYS, clippedStart: true, clippedEnd: true })
  })

  it('raccourcit une absence terminée plus tôt et ignore une absence annulée avant son début', () => {
    const ended = absence('fin', '1', ['2026-10-15', '2026-10-30'], '2026-10-17T08:00:00Z')
    const cancelled = absence('annulee', '2', ['2026-10-25', '2026-10-30'], '2026-10-18T08:00:00Z')
    const rows = timelineRows([ended, cancelled], DAYS, NOW)
    expect(rows).toHaveLength(1)
    expect(rows[0]?.bars[0]).toMatchObject({ state: 'past', start: 3, span: 3 })
    expect(absenceState(ended, NOW)).toBe('past')
  })
})

describe('navigation au clavier', () => {
  const rows = timelineRows(
    [
      absence('a1', '1', ['2026-10-13', '2026-10-14']),
      absence('a2', '1', ['2026-10-27', '2026-10-28']),
      absence('b1', '2', ['2026-10-12', '2026-10-12']),
      absence('b2', '2', ['2026-10-26', '2026-10-26']),
    ],
    DAYS,
    NOW,
  )

  it('passe d’une barre à l’autre dans une ligne', () => {
    expect(moveFocus(rows, { row: 0, bar: 0 }, 'ArrowRight')).toStrictEqual({ row: 0, bar: 1 })
    expect(moveFocus(rows, { row: 0, bar: 1 }, 'ArrowRight')).toBeNull()
    expect(moveFocus(rows, { row: 0, bar: 1 }, 'Home')).toStrictEqual({ row: 0, bar: 0 })
    expect(moveFocus(rows, { row: 0, bar: 0 }, 'End')).toStrictEqual({ row: 0, bar: 1 })
  })

  it('descend vers la barre la plus proche de la ligne suivante', () => {
    expect(moveFocus(rows, { row: 0, bar: 1 }, 'ArrowDown')).toStrictEqual({ row: 1, bar: 1 })
    expect(moveFocus(rows, { row: 1, bar: 0 }, 'ArrowUp')).toStrictEqual({ row: 0, bar: 0 })
    expect(moveFocus(rows, { row: 1, bar: 0 }, 'ArrowDown')).toBeNull()
    expect(moveFocus(rows, { row: 0, bar: 0 }, 'Enter')).toBeNull()
  })
})
