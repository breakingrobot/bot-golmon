// Page des présences servie par la maquette de l'API.

import { screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { MOCK_GUILD, renderApp } from '@/test/app'

const TIMEOUT = { timeout: 5000 }

describe('page des présences (maquette)', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_MOCK', '1')
  })

  it('liste les présences et ouvre le détail dans un panneau', async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${MOCK_GUILD}/presences`)

    expect(await screen.findByRole('heading', { level: 1, name: 'Présences' }, TIMEOUT)).toBeVisible()
    const list = await screen.findByRole('region', { name: 'Liste des présences' }, TIMEOUT)
    const [first] = await within(list).findAllByRole('button', { name: /Run Elysian/u }, TIMEOUT)
    if (!first) throw new Error('présence absente')
    await user.click(first)

    await waitFor(() => expect(router.state.location.search).toMatchObject({ presence: expect.any(String) }))
    const sheet = await screen.findByRole('dialog', {}, TIMEOUT)
    expect(await within(sheet).findByRole('heading', { name: /Présents · \d+/u }, TIMEOUT)).toBeVisible()
    expect(within(sheet).getByRole('button', { name: 'Clôturer' })).toBeVisible()
  })

  it("affiche l'assiduité de la période choisie", async () => {
    const user = userEvent.setup()
    const router = renderApp(`/serveurs/${MOCK_GUILD}/presences?onglet=assiduite`)

    expect(await screen.findByRole('heading', { name: 'Classement' }, TIMEOUT)).toBeVisible()
    expect(screen.getByRole('link', { name: /Exporter en CSV/u })).toHaveAttribute(
      'href',
      `/api/guilds/${MOCK_GUILD}/presences/export.csv?jours=30`,
    )
    await user.click(screen.getByRole('radio', { name: '90 jours' }))
    await waitFor(() => expect(router.state.location.search).toMatchObject({ onglet: 'assiduite', jours: '90' }))
  })
})
