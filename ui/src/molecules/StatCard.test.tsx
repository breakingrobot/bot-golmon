import { render, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it } from 'vitest'

import { StatCard } from '@/molecules/StatCard'

const renderInList = (card: ReactNode) => render(<dl>{card}</dl>)

describe('StatCard', () => {
  it('associe le libellé, la valeur et la précision', () => {
    renderInList(<StatCard label="Taux de présence" value="78 %" hint="moyenne, 12 semaines" />)

    expect(screen.getByRole('term')).toHaveTextContent('Taux de présence')
    const definitions = screen.getAllByRole('definition')
    expect(definitions[0]).toHaveTextContent('78 %')
    expect(definitions[1]).toHaveTextContent('moyenne, 12 semaines')
  })

  it("n'affiche pas de précision vide", () => {
    renderInList(<StatCard label="Membres suivis" value={14} />)
    expect(screen.getAllByRole('definition')).toHaveLength(1)
  })

  it('colore la valeur selon le ton', () => {
    renderInList(<StatCard label="Sans réponse" value={1} tone="danger" />)
    expect(screen.getByRole('definition')).toHaveClass('text-danger')
  })
})
