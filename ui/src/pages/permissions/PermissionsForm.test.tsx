import { screen, waitFor, within } from '@testing-library/react'
import { userEvent } from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { PermissionsResponse } from '@/api/types'
import { PermissionsForm } from '@/pages/permissions/PermissionsForm'
import { jsonResponse, renderInGuild, TEST_GUILD } from '@/test/render'

const DATA: PermissionsResponse = {
  roles: { member: { id: 'r-membre', name: 'Membre', color: 0x34_98_db }, grade: null, lead: null },
  guildRoles: [{ id: 'r-membre', name: 'Membre', color: 0x34_98_db }],
  actions: [
    {
      key: 'contact.voir',
      module: 'Contacts',
      label: 'Consulter le répertoire',
      defaultLevel: 'membre',
      level: 'membre',
      strict: false,
    },
    {
      key: 'contact.modifier',
      module: 'Contacts',
      label: 'Modifier et supprimer',
      defaultLevel: 'grade',
      level: 'lead',
      strict: false,
    },
  ],
}

const levelSelect = (label: string) => screen.getByRole('combobox', { name: `Niveau requis : ${label}` })

describe('PermissionsForm', () => {
  it('propose les quatre niveaux, « Membres du groupe » en premier', async () => {
    const user = userEvent.setup()
    renderInGuild(<PermissionsForm data={DATA} />)

    await user.click(levelSelect('Consulter le répertoire'))
    const options = await screen.findAllByRole('option')

    expect(options.map((option) => option.textContent)).toStrictEqual([
      expect.stringContaining('Membres du groupe'),
      expect.stringContaining('Gradés'),
      expect.stringContaining('Lead'),
      expect.stringContaining('Administrateurs'),
    ])
    expect(screen.queryByRole('option', { name: /Tout le monde/u })).not.toBeInTheDocument()
  })

  it("n'envoie que les niveaux modifiés, le retour au défaut valant null", async () => {
    const fetchMock = vi.fn<typeof fetch>(() => jsonResponse(DATA))
    vi.stubGlobal('fetch', fetchMock)
    const user = userEvent.setup()
    renderInGuild(<PermissionsForm data={DATA} />)

    const save = screen.getByRole('button', { name: /^Enregistrer/u })
    expect(save).toBeDisabled()

    // Monter « Consulter » d'un cran, ramener « Modifier » à son niveau par défaut.
    await user.click(levelSelect('Consulter le répertoire'))
    await user.click(await screen.findByRole('option', { name: /Gradés/u }))
    const row = screen.getByText('Modifier et supprimer').closest('tr')
    if (!row) throw new Error('ligne absente')
    await user.click(within(row).getByRole('button', { name: /Revenir au niveau par défaut/u }))

    expect(save).toHaveTextContent('Enregistrer (2)')
    await user.click(save)

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(`/api/guilds/${TEST_GUILD.id}/permissions`, expect.anything()),
    )
    const init = fetchMock.mock.calls.find(([url]) => String(url).endsWith('/permissions'))?.[1]
    expect(init?.method).toBe('PUT')
    expect(JSON.parse(String(init?.body))).toStrictEqual({
      levels: { 'contact.voir': 'grade', 'contact.modifier': null },
    })
  })
})
