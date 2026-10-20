import { describe, expect, it } from 'vitest'

import {
  fallbackItemLabel,
  inventoryDiff,
  isPropertyId,
  normalizeItem,
  parseInventoryList,
  parseQuantity,
} from '@/lib/coffre-inventory'

describe('normalizeItem', () => {
  it('écrit les identifiants comme le bot', () => {
    expect(normalizeItem('  Repair Kit ')).toBe('repair_kit')
    expect(normalizeItem('ammo-9mm.v2')).toBe('ammo-9mm.v2')
  })

  it('refuse les caractères hors du jeu et les noms vides ou trop longs', () => {
    expect(normalizeItem('kit/réparation')).toBeNull()
    expect(normalizeItem('   ')).toBeNull()
    expect(normalizeItem('x'.repeat(65))).toBeNull()
  })
})

describe('parseQuantity et isPropertyId', () => {
  it('lit les entiers de 0 à 1 000 000, espaces de milliers admis', () => {
    expect(parseQuantity('2 790')).toBe(2790)
    expect(parseQuantity('0')).toBe(0)
    expect(parseQuantity('1000000')).toBe(1_000_000)
    expect(parseQuantity('1000001')).toBeNull()
  })

  it('refuse les quantités négatives, décimales ou vides', () => {
    expect(parseQuantity('-3')).toBeNull()
    expect(parseQuantity('4,5')).toBeNull()
    expect(parseQuantity('')).toBeNull()
  })

  it('reconnaît un numéro de propriété', () => {
    expect(isPropertyId(' 8740 ')).toBe(true)
    expect(isPropertyId('87a0')).toBe(false)
    expect(isPropertyId('')).toBe(false)
  })
})

describe('parseInventoryList', () => {
  it('lit la grammaire de la commande Discord', () => {
    expect(parseInventoryList('repair_kit:42, acetone 2790; x=1')).toStrictEqual({
      ok: true,
      entries: [
        { item: 'repair_kit', quantity: 42 },
        { item: 'acetone', quantity: 2790 },
        { item: 'x', quantity: 1 },
      ],
    })
  })

  it('accepte les retours à la ligne, les espaces dans le nom et les milliers', () => {
    expect(parseInventoryList('Repair Kit : 1 200\n\nmedikit=0\n')).toStrictEqual({
      ok: true,
      entries: [
        { item: 'repair_kit', quantity: 1200 },
        { item: 'medikit', quantity: 0 },
      ],
    })
  })

  it('lit les inventaires collés depuis le jeu, comme le bot', () => {
    const pasted =
      '- repair_kit x42\n• 2790x acetone\n* 650 × red_powder\r\nlockpick\t12\nmedikit×3\n\n— 5 bandage\n· weapon_pistol x 1 200'
    expect(parseInventoryList(pasted)).toStrictEqual({
      ok: true,
      entries: [
        { item: 'repair_kit', quantity: 42 },
        { item: 'acetone', quantity: 2790 },
        { item: 'red_powder', quantity: 650 },
        { item: 'lockpick', quantity: 12 },
        { item: 'medikit', quantity: 3 },
        { item: 'bandage', quantity: 5 },
        { item: 'weapon_pistol', quantity: 1200 },
      ],
    })
  })

  it('ne prend pas un nom finissant par x pour un multiplicateur', () => {
    expect(parseInventoryList('box 4')).toStrictEqual({ ok: true, entries: [{ item: 'box', quantity: 4 }] })
    expect(parseInventoryList('42 X Repair Kit')).toStrictEqual({
      ok: true,
      entries: [{ item: 'repair_kit', quantity: 42 }],
    })
    expect(parseInventoryList('4x4_kit 3')).toStrictEqual({ ok: true, entries: [{ item: '4x4_kit', quantity: 3 }] })
    expect(parseInventoryList('-1 x kit').ok).toBe(false)
    expect(parseInventoryList('repair_kit x beaucoup')).toStrictEqual({
      ok: false,
      error: '« repair_kit x beaucoup » : quantité illisible.',
    })
  })

  it('garde la dernière quantité d’un objet répété', () => {
    expect(parseInventoryList('lockpick:3, lockpick:5')).toStrictEqual({
      ok: true,
      entries: [{ item: 'lockpick', quantity: 5 }],
    })
  })

  it('signale la première entrée illisible', () => {
    expect(parseInventoryList('repair_kit:42, acetone')).toStrictEqual({
      ok: false,
      error: '« acetone » : écris objet:quantité, par exemple repair_kit:42.',
    })
    expect(parseInventoryList('repair_kit:beaucoup')).toStrictEqual({
      ok: false,
      error: '« repair_kit:beaucoup » : quantité illisible.',
    })
    expect(parseInventoryList('kit/x:4')).toStrictEqual({ ok: false, error: '« kit/x:4 » : nom d’objet invalide.' })
    expect(parseInventoryList('medikit:-2')).toStrictEqual({
      ok: false,
      error: '« medikit:-2 » : quantité entre 0 et 1000000.',
    })
    expect(parseInventoryList(' , ; ')).toStrictEqual({
      ok: false,
      error: 'Indique au moins un objet, par exemple repair_kit:42.',
    })
  })
})

describe('inventoryDiff', () => {
  const known = [
    { item: 'repair_kit', label: 'Kit de réparation', quantity: -12 },
    { item: 'lockpick', label: 'Crochet', quantity: 32 },
  ]

  it('ne garde que les objets qui changent, avec leur écart signé', () => {
    expect(
      inventoryDiff(known, [
        { item: 'repair_kit', quantity: 22 },
        { item: 'lockpick', quantity: 32 },
        { item: 'acetone', quantity: 5 },
        { item: 'medikit', quantity: 0 },
      ]),
    ).toStrictEqual([
      { item: 'repair_kit', label: 'Kit de réparation', before: -12, after: 22, delta: 34 },
      { item: 'acetone', label: 'Acetone', before: 0, after: 5, delta: 5 },
    ])
  })

  it('compte un objet saisi deux fois avec sa dernière quantité', () => {
    expect(
      inventoryDiff(known, [
        { item: 'lockpick', quantity: 29 },
        { item: 'lockpick', quantity: 32 },
      ]),
    ).toStrictEqual([])
    expect(inventoryDiff(known, [{ item: 'lockpick', quantity: 29 }], () => '?')).toStrictEqual([
      { item: 'lockpick', label: 'Crochet', before: 32, after: 29, delta: -3 },
    ])
  })

  it('nomme un objet inconnu comme le bot', () => {
    expect(fallbackItemLabel('weapon_pistol-mk2')).toBe('Weapon pistol mk2')
  })
})
