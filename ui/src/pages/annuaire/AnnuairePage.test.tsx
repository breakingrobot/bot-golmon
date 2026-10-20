// Page de l'annuaire servie par la maquette de l'API.

import { screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { normalizePlate } from '@/pages/annuaire/AddVehicleDialog'
import { validPhone } from '@/pages/annuaire/PhoneDialog'
import { MOCK_GUILD, renderApp } from '@/test/app'

const TIMEOUT = { timeout: 5000 }

describe("page de l'annuaire (maquette)", () => {
  beforeEach(() => {
    vi.stubEnv('VITE_MOCK', '1')
  })

  it('cherche un véhicule par plaque', async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${MOCK_GUILD}/annuaire`)

    expect(await screen.findByRole('heading', { name: 'Véhicules' }, TIMEOUT)).toBeVisible()
    await user.type(screen.getByRole('searchbox', { name: 'Recherche' }), '88 mec')
    await waitFor(() => expect(router.state.location.search).toStrictEqual({ q: '88 mec' }), TIMEOUT)
    await waitFor(() => expect(screen.getAllByText('88MEC01').length).toBeGreaterThan(0), TIMEOUT)
    await waitFor(() => expect(screen.queryAllByText('42CRB13')).toHaveLength(0), TIMEOUT)
  })

  it('enregistre son numéro dans l’onglet Numéros', async () => {
    const user = userEvent.setup()
    renderApp(`/serveurs/${MOCK_GUILD}/annuaire?onglet=numeros`)

    await user.click(await screen.findByRole('button', { name: 'Modifier mon numéro' }, TIMEOUT))
    const dialog = await screen.findByRole('dialog')
    const input = within(dialog).getByLabelText('Numéro')
    await user.clear(input)
    await user.type(input, '555-9876')
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument(), TIMEOUT)
    expect(await screen.findByText('555-9876', {}, TIMEOUT)).toBeVisible()
  })
})

describe('saisies de l’annuaire', () => {
  it('normalise les plaques et vérifie les numéros comme le bot', () => {
    expect(normalizePlate(' d6cs-3p0g ')).toBe('D6CS3P0G')
    expect(validPhone('555-1234')).toBe(true)
    expect(validPhone('5551234567')).toBe(false)
    expect(validPhone('556-1234')).toBe(false)
  })
})
