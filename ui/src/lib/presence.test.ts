import { describe, expect, it } from 'vitest'

import { answeredShare, normalizeTime, presenceState } from '@/lib/presence'

const NOW = Date.parse('2026-10-20T12:00:00Z')

describe('présences', () => {
  it("donne l'état d'une présence", () => {
    expect(presenceState({ date: '2026-10-20T19:00:00Z', closedAt: null }, NOW)).toBe('upcoming')
    expect(presenceState({ date: '2026-10-20T11:00:00Z', closedAt: null }, NOW)).toBe('open')
    expect(presenceState({ date: '2026-10-21T19:00:00Z', closedAt: '2026-10-20T11:00:00Z' }, NOW)).toBe('closed')
  })

  it('normalise une heure comme le bot', () => {
    expect(normalizeTime('21:00')).toBe('21:00')
    expect(normalizeTime(' 9h30 ')).toBe('09:30')
    expect(normalizeTime('25:00')).toBeNull()
    expect(normalizeTime('21')).toBeNull()
    expect(normalizeTime('21:5')).toBeNull()
  })

  it('calcule la part de réponses', () => {
    expect(answeredShare({ present: 3, absent: 1, retard: 0, noAnswer: 4 })).toBe(50)
    expect(answeredShare({ present: 0, absent: 0, retard: 0, noAnswer: 0 })).toBeNull()
  })
})
