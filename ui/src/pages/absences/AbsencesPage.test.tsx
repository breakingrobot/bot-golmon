// Page des absences servie par la maquette de l'API, et validation du dialogue.

import { screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { validateAbsence } from '@/pages/absences/DeclareAbsenceDialog'
import { MOCK_GUILD, renderApp } from '@/test/app'

const TIMEOUT = { timeout: 5000 }

describe('page des absences (maquette)', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_MOCK', '1')
  })

  it('montre la frise et passe d’une absence à l’autre au clavier', async () => {
    const user = userEvent.setup()
    renderApp(`/serveurs/${MOCK_GUILD}/absences`)

    const timeline = await screen.findByRole('group', { name: /^Absences du/u }, TIMEOUT)
    const bars = within(timeline).getAllByRole('button')
    expect(bars.length).toBeGreaterThan(2)
    // Une seule barre atteignable par Tab, les flèches déplacent le focus.
    expect(bars.filter((bar) => bar.tabIndex === 0)).toHaveLength(1)
    bars[0]?.focus()
    await user.keyboard('{ArrowDown}')
    expect(document.activeElement).not.toBe(bars[0])
    expect(document.activeElement?.getAttribute('tabindex')).toBe('0')
  })

  it('déclare une absence et la liste', async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${MOCK_GUILD}/absences?vue=liste`)

    await screen.findByRole('heading', { name: 'En cours' }, TIMEOUT)
    await user.click(screen.getByRole('button', { name: 'Déclarer une absence' }))
    const dialog = await screen.findByRole('dialog')
    await user.type(within(dialog).getByLabelText('Motif'), 'Vacances au soleil')
    await user.type(within(dialog).getByLabelText('Dernier jour'), '2099-08-20')
    await user.click(within(dialog).getByRole('button', { name: 'Déclarer' }))

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument(), TIMEOUT)
    expect(await screen.findByText('Vacances au soleil', {}, TIMEOUT)).toBeVisible()
    expect(router.state.location.search).toStrictEqual({ vue: 'liste' })
  })
})

describe('validation de la déclaration', () => {
  const form = { motif: 'x', from: '2026-10-20', to: '2026-10-22', forOther: false, member: null }

  it('reprend les règles de /absence declare', () => {
    expect(validateAbsence(form, '2026-10-20')).toStrictEqual({})
    expect(validateAbsence({ ...form, motif: '  ' }, '2026-10-20').motif).toBe('Indiquez un motif.')
    expect(validateAbsence({ ...form, to: '2026-10-19' }, '2026-10-20').to).toBe('Cette absence est déjà terminée.')
    expect(validateAbsence({ ...form, from: '2026-10-25' }, '2026-10-20').to).toContain('suivre')
    expect(validateAbsence({ ...form, forOther: true }, '2026-10-20').member).toBe('Choisissez le membre.')
  })
})
