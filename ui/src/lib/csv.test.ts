import { describe, expect, it } from 'vitest'

import { csvCell, csvDateTime, csvDay, toCsv } from '@/lib/csv'

describe('export CSV', () => {
  it('échappe comme RFC 4180, avec le séparateur ;', () => {
    expect(csvCell('simple')).toBe('simple')
    expect(csvCell('a;b')).toBe('"a;b"')
    expect(csvCell('il a dit "oui"')).toBe('"il a dit ""oui"""')
    expect(csvCell('deux\nlignes')).toBe('"deux\nlignes"')
    expect(csvCell(null)).toBe('')
  })

  it('neutralise les formules, comme les exports du bot', () => {
    expect(csvCell('=HYPERLINK("x";vente)')).toBe('"\'=HYPERLINK(""x"";vente)"')
    expect(csvCell('-3')).toBe("'-3")
    expect(csvCell('@Elodie')).toBe("'@Elodie")
  })

  it('écrit les nombres pour un Excel français', () => {
    expect(csvCell(1500)).toBe('1500')
    expect(csvCell(12.5)).toBe('12,5')
    expect(csvCell(-3)).toBe('-3')
    expect(csvCell(Number.NaN)).toBe('')
  })

  it('assemble BOM, titres et lignes en CRLF', () => {
    const csv = toCsv(
      [
        { name: 'Les Ballas', count: 2 },
        { name: 'Vagos; Est', count: 0 },
      ],
      [
        { header: 'Nom', value: (row) => row.name },
        { header: 'Notes', value: (row) => row.count },
      ],
    )
    expect(csv).toBe('﻿Nom;Notes\r\nLes Ballas;2\r\n"Vagos; Est";0\r\n')
  })

  it('date et heure de Paris, jours', () => {
    expect(csvDateTime('2026-10-18T17:20:13Z')).toBe('18/10/2026 19:20')
    expect(csvDateTime('2026-01-05T23:30:00Z')).toBe('06/01/2026 00:30')
    expect(csvDay('2026-10-04')).toBe('04/10/2026')
  })
})
