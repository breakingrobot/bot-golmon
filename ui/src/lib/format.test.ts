import { describe, expect, it } from 'vitest'

import {
  formatCompactMoney,
  formatDay,
  formatMoney,
  formatMonth,
  formatNumber,
  formatPercent,
  formatRelative,
  formatShortDay,
  formatSignedMoney,
  formatWeek,
  plural,
} from '@/lib/format'

// Espace insécable : les espaces fines du format français sont remplacées.
const NBSP = '\u00A0'

describe('montants et nombres', () => {
  it('écrit les montants à la française, « 12 500 $ »', () => {
    expect(formatMoney(12_500)).toBe(`12${NBSP}500${NBSP}$`)
    expect(formatMoney(0)).toBe(`0${NBSP}$`)
  })

  it("garde les décimales seulement si on les demande et qu'elles existent", () => {
    expect(formatMoney(1234.5, true)).toBe(`1${NBSP}234,50${NBSP}$`)
    expect(formatMoney(1234, true)).toBe(`1${NBSP}234${NBSP}$`)
    expect(formatMoney(1234.5)).toBe(`1${NBSP}235${NBSP}$`)
  })

  it('signe les montants avec le vrai signe moins', () => {
    expect(formatSignedMoney(1200)).toBe(`+1${NBSP}200${NBSP}$`)
    expect(formatSignedMoney(-300)).toBe(`−300${NBSP}$`)
    expect(formatSignedMoney(0)).toBe(`0${NBSP}$`)
  })

  it('abrège les montants des axes', () => {
    expect(formatCompactMoney(12_500)).toBe(`12,5${NBSP}k${NBSP}$`)
    expect(formatCompactMoney(250_000)).toBe(`250${NBSP}k${NBSP}$`)
  })

  it('formate entiers, pourcentages et pluriels', () => {
    expect(formatNumber(1_234_567)).toBe(`1${NBSP}234${NBSP}567`)
    expect(formatPercent(78)).toBe(`78${NBSP}%`)
    expect(plural(1, 'membre')).toBe(`1${NBSP}membre`)
    expect(plural(3, 'membre')).toBe(`3${NBSP}membres`)
    expect(plural(2, 'travail', 'travaux')).toBe(`2${NBSP}travaux`)
  })

  it("n'utilise jamais d'espace fine (rendu inégal selon les polices)", () => {
    expect(formatMoney(1_000_000)).not.toContain('\u202F')
    expect(formatCompactMoney(1_500_000)).not.toContain('\u202F')
  })
})

describe('dates', () => {
  it('affiche un jour calendaire sans décalage de fuseau', () => {
    expect(formatShortDay('2026-10-05')).toBe('5 oct.')
    expect(formatDay('2026-01-01')).toBe('1 janv. 2026')
  })

  it('ramène un instant ISO au jour de Paris', () => {
    // 23 h 30 UTC le 4 octobre = 5 octobre à Paris (UTC+2).
    expect(formatShortDay('2026-10-04T23:30:00Z')).toBe('5 oct.')
  })

  it('écrit les mois courts et longs', () => {
    expect(formatMonth('2026-10')).toBe('oct. 26')
    expect(formatMonth('2026-10', true)).toBe('octobre 2026')
  })

  it('nomme les semaines par leur premier jour', () => {
    expect(formatWeek('2026-09-28')).toBe('Sem. du 28 sept.')
  })

  it('rend les dates invalides telles quelles', () => {
    expect(formatShortDay('bientôt')).toBe('bientôt')
  })

  it('exprime les durées relatives', () => {
    const now = Date.parse('2026-10-19T12:00:00Z')
    expect(formatRelative('2026-10-19T11:00:00Z', now)).toBe('il y a 1 heure')
    expect(formatRelative('2026-10-18T12:00:00Z', now)).toBe('hier')
    expect(formatRelative('2026-10-19T12:00:30Z', now)).toBe('dans 30 secondes')
  })
})
