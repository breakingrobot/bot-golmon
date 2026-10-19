import { describe, expect, it } from 'vitest'

import { ALL, compact, fromSelect, searchDay, searchOneOf, searchPage, searchString } from '@/lib/search'

describe("paramètres d'adresse", () => {
  it('nettoie les chaînes et ignore les valeurs vides', () => {
    expect(searchString('  Ballas  ')).toBe('Ballas')
    expect(searchString('   ')).toBeUndefined()
    expect(searchString(42)).toBe('42')
    expect(searchString(null)).toBeUndefined()
    expect(searchString({ q: 'x' })).toBeUndefined()
  })

  it('tronque les chaînes trop longues', () => {
    expect(searchString('a'.repeat(500))).toHaveLength(200)
  })

  it('lit les numéros de page, en texte ou en nombre', () => {
    expect(searchPage('3')).toBe(3)
    expect(searchPage(2)).toBe(2)
  })

  it.each(['1', '0', '-2', '2.5', 'abc', null])('ignore la page %s (première page par défaut)', (value) => {
    expect(searchPage(value)).toBeUndefined()
  })

  it('valide les jours AAAA-MM-JJ', () => {
    expect(searchDay('2026-10-19')).toBe('2026-10-19')
    expect(searchDay('19/10/2026')).toBeUndefined()
  })

  it('restreint une valeur à une liste', () => {
    const statuses = ['open', 'closed'] as const
    expect(searchOneOf('open', statuses)).toBe('open')
    expect(searchOneOf('archived', statuses)).toBeUndefined()
  })

  it('retire les clés indéfinies (exactOptionalPropertyTypes)', () => {
    const result = compact({ q: 'x', page: undefined, type: 'deposit' })
    expect(result).toStrictEqual({ q: 'x', type: 'deposit' })
    expect(Object.hasOwn(result, 'page')).toBe(false)
  })

  it('traduit la valeur « tous » des listes déroulantes', () => {
    expect(fromSelect(ALL)).toBeUndefined()
    expect(fromSelect('ALLIE')).toBe('ALLIE')
  })
})
