import { describe, expect, it } from 'vitest'

import { LIVE_TOPICS, isLiveMessage, queryKeysFor, reconnectDelay, TOPIC_DOMAINS } from '@/lib/live'

const lowest = () => 0

describe('mise à jour en direct', () => {
  it('reconnaît les sujets du serveur', () => {
    expect(isLiveMessage('coffres')).toBe(true)
    expect(isLiveMessage('*')).toBe(true)
    expect(isLiveMessage('inconnu')).toBe(false)
    expect(Object.keys(TOPIC_DOMAINS).toSorted()).toStrictEqual([...LIVE_TOPICS].toSorted())
  })

  it('un sujet recharge ses domaines, sous le serveur', () => {
    const keys = queryKeysFor('42', 'cotisations')
    expect(keys).toContainEqual(['guild', '42', 'cotisations'])
    expect(keys).toContainEqual(['guild', '42', 'overview'])
    expect(keys.every((key) => key[0] === 'guild' && key[1] === '42')).toBe(true)
    expect(queryKeysFor('42', 'coffres')).toContainEqual(['guild', '42', 'coffres'])
  })

  it('la configuration recharge aussi les droits ; `*` recharge tout', () => {
    expect(queryKeysFor('42', 'config')).toContainEqual(['me'])
    expect(queryKeysFor('42', 'journal')).not.toContainEqual(['me'])
    expect(queryKeysFor('42', '*')).toStrictEqual([['guild', '42'], ['me']])
  })

  it('reconnexion : attente croissante, plafonnée', () => {
    expect(reconnectDelay(0, lowest)).toBe(500)
    expect(reconnectDelay(3, lowest)).toBe(4000)
    expect(reconnectDelay(20, () => 1)).toBe(30_000)
  })
})
