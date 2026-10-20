// Page des recrues servie par la maquette de l'API.

import { screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { MOCK_GUILD, renderApp } from '@/test/app'

const TIMEOUT = { timeout: 5000 }

describe('page des recrues (maquette)', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_MOCK', '1')
  })

  it('refuse un avis sans motif, puis l’enregistre', async () => {
    const user = userEvent.setup()
    renderApp(`/serveurs/${MOCK_GUILD}/recrues`)

    const [give] = await screen.findAllByRole('button', { name: 'Donner mon avis' }, TIMEOUT)
    if (!give) throw new Error('bouton absent')
    await user.click(give)
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('radio', { name: /Pour/u }))
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    expect(await within(dialog).findByText('Le motif est obligatoire.')).toBeVisible()

    await user.type(within(dialog).getByLabelText('Motif'), 'Toujours présent aux sorties.')
    await user.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument(), TIMEOUT)
    expect(await screen.findByText(/^Votre avis/u, {}, TIMEOUT)).toHaveTextContent('Toujours présent aux sorties.')
  })

  it('ouvre l’historique d’une recrue', async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${MOCK_GUILD}/recrues`)

    const [history] = await screen.findAllByRole('button', { name: 'Historique' }, TIMEOUT)
    if (!history) throw new Error('bouton absent')
    await user.click(history)
    await waitFor(() => expect(router.state.location.search).toMatchObject({ recrue: expect.any(String) }))
    expect(await screen.findByRole('dialog', { name: /Historique des avis/u }, TIMEOUT)).toBeVisible()
  })
})
