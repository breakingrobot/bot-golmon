import { describe, expect, it } from 'vitest'

import { fold, matchesQuery } from '@/lib/text'

describe('recherche sans accents', () => {
  it('ramène accents, ligatures et majuscules', () => {
    expect(fold('  Élodie Cœur ')).toBe('elodie coeur')
    expect(fold('Propriété')).toBe('propriete')
  })

  it('exige chaque mot, dans n’importe quel texte', () => {
    expect(matchesQuery('elo', 'Élodie')).toBe(true)
    expect(matchesQuery('kit repar', 'Kit de réparation', 'repair_kit')).toBe(true)
    expect(matchesQuery('kit pain', 'Kit de réparation')).toBe(false)
    expect(matchesQuery('   ', 'n’importe quoi')).toBe(true)
    expect(matchesQuery('x', null)).toBe(false)
  })
})
