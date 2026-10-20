import { describe, expect, it } from 'vitest'

import { choiceOf, trend, trialLength } from '@/lib/recrue'

const NOW = Date.parse('2026-10-20T12:00:00Z')

const votes = (pour: number, contre: number, blanc: number) => ({ pour, contre, blanc, veto: null })

describe("ancienneté d'essai", () => {
  it('compte en jours', () => {
    expect(trialLength('2026-10-20T08:00:00Z', NOW)).toBe("moins d'un jour")
    expect(trialLength('2026-10-19T08:00:00Z', NOW)).toBe('1 jour')
    expect(trialLength('2026-10-15T08:00:00Z', NOW)).toBe('5 jours')
  })

  it('puis en semaines', () => {
    expect(trialLength('2026-10-13T08:00:00Z', NOW)).toBe('1 semaine')
    expect(trialLength('2026-10-04T08:00:00Z', NOW)).toBe('2 semaines et 2 jours')
    // Date future (horloge décalée) : jamais négatif.
    expect(trialLength('2026-10-21T08:00:00Z', NOW)).toBe("moins d'un jour")
  })
})

describe('tendance des avis', () => {
  it('le véto l’emporte, puis la majorité', () => {
    expect(trend({ ...votes(5, 0, 0), veto: { by: { id: '1', name: 'Lead', avatarUrl: null }, motif: 'x' } })).toBe(
      'veto',
    )
    expect(trend(votes(0, 0, 0))).toBe('aucun')
    expect(trend(votes(3, 1, 2))).toBe('favorable')
    expect(trend(votes(1, 2, 0))).toBe('defavorable')
    expect(trend(votes(1, 1, 3))).toBe('partage')
  })

  it('donne le libellé de chaque avis', () => {
    expect(choiceOf('CONTRE')).toMatchObject({ label: 'Contre', emoji: '❌' })
  })
})
